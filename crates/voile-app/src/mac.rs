//! Intégration macOS, adaptée de l'exemple `cefsimple` de cef-rs :
//! * `VoileApplication` : sous-classe de `NSApplication` conforme à
//!   `CefAppProtocol` (obligatoire pour Chromium) ;
//! * `-terminate:` (Cmd+Q, Dock, fermeture de session) passe par le shell
//!   pour fermer proprement les navigateurs ;
//! * un menu principal minimal : sans menu Édition, Cmd+C/Cmd+V/Cmd+Z ne
//!   fonctionnent pas dans les pages.

use crate::bridge::{self, Event};
use crate::shell::UiAction;
use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::{
    define_class, extern_methods, msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool, NSObject, NSObjectProtocol, ProtocolObject, Sel},
    sel, ClassType, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSApp, NSApplication, NSApplicationDelegate, NSApplicationTerminateReply, NSEvent, NSMenu,
    NSMenuItem, NSWindowAnimationBehavior,
};
use objc2_foundation::{ns_string, NSString};
use std::cell::Cell;

define_class! {
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    pub struct VoileAppDelegate;

    unsafe impl NSObjectProtocol for VoileAppDelegate {}

    // Actions du menu Fichier. macOS garde certains raccourcis pour lui
    // (Cmd+W n'arrive jamais à la page) : seul un élément de menu les reçoit.
    impl VoileAppDelegate {
        #[unsafe(method(voileNewTab:))]
        fn new_tab(&self, _sender: Option<&AnyObject>) {
            bridge::send(Event::Action(UiAction::FocusCommandBar { new_tab: true }));
        }

        #[unsafe(method(voileOpenLocation:))]
        fn open_location(&self, _sender: Option<&AnyObject>) {
            bridge::send(Event::Action(UiAction::FocusCommandBar { new_tab: false }));
        }

        #[unsafe(method(voileCloseTab:))]
        fn close_tab(&self, _sender: Option<&AnyObject>) {
            bridge::send(Event::Action(UiAction::CloseTab));
        }

        #[unsafe(method(voileReopenTab:))]
        fn reopen_tab(&self, _sender: Option<&AnyObject>) {
            bridge::send(Event::Action(UiAction::ReopenClosed));
        }

        #[unsafe(method(voileToggleSidebar:))]
        fn toggle_sidebar(&self, _sender: Option<&AnyObject>) {
            bridge::send(Event::Action(UiAction::ToggleSidebar));
        }
    }

    unsafe impl NSApplicationDelegate for VoileAppDelegate {
        #[unsafe(method(applicationShouldTerminate:))]
        unsafe fn application_should_terminate(&self, _sender: &NSApplication) -> NSApplicationTerminateReply {
            NSApplicationTerminateReply::TerminateNow
        }

        /// Clic sur l'icône du Dock alors que Voile tourne déjà.
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        unsafe fn application_should_handle_reopen(&self, _sender: &NSApplication, _visible: Bool) -> Bool {
            bridge::send(Event::Action(UiAction::Raise));
            Bool::NO
        }

        #[unsafe(method(applicationSupportsSecureRestorableState:))]
        unsafe fn application_supports_secure_restorable_state(&self, _sender: &NSApplication) -> Bool {
            Bool::YES
        }
    }
}

impl VoileAppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = VoileAppDelegate::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

#[derive(Default)]
pub struct VoileApplicationIvars {
    handling_send_event: Cell<Bool>,
}

define_class!(
    #[unsafe(super(NSApplication))]
    #[ivars = VoileApplicationIvars]
    pub struct VoileApplication;

    impl VoileApplication {
        #[unsafe(method(sendEvent:))]
        unsafe fn send_event(&self, event: &NSEvent) {
            let was_sending = self.is_handling_send_event();
            if !was_sending {
                self.set_handling_send_event(true);
            }
            let _: () = msg_send![super(self), sendEvent: event];
            if !was_sending {
                self.set_handling_send_event(false);
            }
        }

        /// Chromium doit quitter sa boucle de messages normalement : on ne
        /// laisse pas Cocoa appeler exit(), on demande au shell de tout fermer.
        #[unsafe(method(terminate:))]
        unsafe fn terminate(&self, _sender: Option<&AnyObject>) {
            bridge::send(Event::Action(UiAction::Quit));
        }
    }

    unsafe impl CrAppControlProtocol for VoileApplication {
        #[unsafe(method(setHandlingSendEvent:))]
        unsafe fn _set_handling_send_event(&self, handling: Bool) {
            self.ivars().handling_send_event.set(handling);
        }
    }

    unsafe impl CrAppProtocol for VoileApplication {
        #[unsafe(method(isHandlingSendEvent))]
        unsafe fn _is_handling_send_event(&self) -> Bool {
            self.ivars().handling_send_event.get()
        }
    }

    unsafe impl CefAppProtocol for VoileApplication {}
);

impl VoileApplication {
    extern_methods! {
        #[unsafe(method(sharedApplication))]
        fn shared_application() -> Retained<Self>;

        #[unsafe(method(setHandlingSendEvent:))]
        fn set_handling_send_event(&self, handling: bool);

        #[unsafe(method(isHandlingSendEvent))]
        fn is_handling_send_event(&self) -> bool;
    }
}

/// À appeler avant toute utilisation de NSApp (donc avant CEF).
pub fn setup_application() {
    let _ = VoileApplication::shared_application();
    let mtm = MainThreadMarker::new().expect("thread principal");
    assert!(NSApp(mtm).isKindOfClass(VoileApplication::class()));
}

/// Changer d'onglet permute deux fenêtres : sans ça macOS fait apparaître la
/// nouvelle en zoom, et l'ancienne se voit en fantôme derrière.
pub fn disable_window_animations() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    for window in NSApp(mtm).windows().iter() {
        window.setAnimationBehavior(NSWindowAnimationBehavior::None);
    }
}

pub fn setup_app_delegate() -> Retained<VoileAppDelegate> {
    let mtm = MainThreadMarker::new().expect("thread principal");
    let delegate = VoileAppDelegate::new(mtm);
    let app = NSApp(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.setMainMenu(Some(&main_menu(mtm)));
    delegate
}

fn item(
    mtm: MainThreadMarker,
    title: &NSString,
    action: Sel,
    key: &NSString,
) -> Retained<NSMenuItem> {
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            title,
            Some(action),
            key,
        )
    }
}

fn submenu(mtm: MainThreadMarker, bar: &NSMenu, title: &NSString, items: &[Retained<NSMenuItem>]) {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), title);
    for i in items {
        menu.addItem(i);
    }
    let holder = NSMenuItem::new(mtm);
    holder.setSubmenu(Some(&menu));
    bar.addItem(&holder);
}

fn main_menu(mtm: MainThreadMarker) -> Retained<NSMenu> {
    let bar = NSMenu::new(mtm);
    submenu(
        mtm,
        &bar,
        ns_string!("Voile"),
        &[
            item(
                mtm,
                ns_string!("Masquer Voile"),
                sel!(hide:),
                ns_string!("h"),
            ),
            item(
                mtm,
                ns_string!("Quitter Voile"),
                sel!(terminate:),
                ns_string!("q"),
            ),
        ],
    );
    submenu(
        mtm,
        &bar,
        ns_string!("Fichier"),
        &[
            item(
                mtm,
                ns_string!("Nouvel onglet"),
                sel!(voileNewTab:),
                ns_string!("t"),
            ),
            item(
                mtm,
                ns_string!("Ouvrir une adresse"),
                sel!(voileOpenLocation:),
                ns_string!("l"),
            ),
            item(
                mtm,
                ns_string!("Fermer l'onglet"),
                sel!(voileCloseTab:),
                ns_string!("w"),
            ),
            item(
                mtm,
                ns_string!("Rouvrir l'onglet fermé"),
                sel!(voileReopenTab:),
                ns_string!("T"),
            ),
            item(
                mtm,
                ns_string!("Afficher ou masquer la sidebar"),
                sel!(voileToggleSidebar:),
                ns_string!("s"),
            ),
        ],
    );
    submenu(
        mtm,
        &bar,
        ns_string!("Édition"),
        &[
            item(mtm, ns_string!("Annuler"), sel!(undo:), ns_string!("z")),
            item(mtm, ns_string!("Rétablir"), sel!(redo:), ns_string!("Z")),
            item(mtm, ns_string!("Couper"), sel!(cut:), ns_string!("x")),
            item(mtm, ns_string!("Copier"), sel!(copy:), ns_string!("c")),
            item(mtm, ns_string!("Coller"), sel!(paste:), ns_string!("v")),
            item(
                mtm,
                ns_string!("Tout sélectionner"),
                sel!(selectAll:),
                ns_string!("a"),
            ),
        ],
    );
    bar
}
