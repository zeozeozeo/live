use geometrydash::{get_base, PlayLayer, PlayerObject, Ptr};
use retour::static_detour;

use crate::bot::BOT;

/// pushButton/releaseButton methods that take [PlayerObject].
type FnPushButton = unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;

/// called when entering a level.
type FnInit = unsafe extern "fastcall" fn(PlayLayer, Ptr, Ptr) -> bool;

/// called when exiting from a level.
type FnQuit = unsafe extern "fastcall" fn(PlayLayer, Ptr);

static_detour! {
    static PushButton: unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;
    static ReleaseButton: unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;
    static Init: unsafe extern "fastcall" fn(PlayLayer, Ptr, Ptr) -> bool;
    static Quit: unsafe extern "fastcall" fn(PlayLayer, Ptr);
}

fn push_button(player: PlayerObject, _edx: Ptr, button: i32) -> bool {
    let res = unsafe { PushButton.call(player, 0, button) };
    log::info!("pushButton: {button}");
    unsafe { BOT.on_action(true, BOT.is_player2_obj(player)) };
    res
}

fn release_button(player: PlayerObject, _edx: Ptr, button: i32) -> bool {
    let res = unsafe { ReleaseButton.call(player, 0, button) };
    log::info!("releaseButton: {button}");
    unsafe { BOT.on_action(false, BOT.is_player2_obj(player)) };
    res
}

fn init(playlayer: PlayLayer, _edx: Ptr, level: Ptr) -> bool {
    let res = unsafe { Init.call(playlayer, 0, level) };

    // update playlayer
    if res {
        unsafe { BOT.playlayer = playlayer };
    }

    res
}

fn quit(playlayer: PlayLayer, _edx: Ptr) {
    unsafe { Quit.call(playlayer, 0) };

    // set playlayer to null
    unsafe { BOT.playlayer = PlayLayer::from_address(0) };
    log::debug!("quit");
}

pub unsafe fn init_hooks() {
    use std::mem::transmute;

    // pushbutton
    let push_button_fn: FnPushButton = transmute(get_base() + 0x1F4E40);
    PushButton
        .initialize(push_button_fn, push_button)
        .expect("failed to hook pushbutton");
    PushButton.enable().expect("failed to enable pushbutton");

    // releasebutton
    let release_button_fn: FnPushButton = transmute(get_base() + 0x1F4F70);
    ReleaseButton
        .initialize(release_button_fn, release_button)
        .expect("failed to hook releasebutton");
    ReleaseButton
        .enable()
        .expect("failed to enable releasebutton");

    // init
    let init_fn: FnInit = transmute(get_base() + 0x01FB780);
    Init.initialize(init_fn, init).expect("failed to hook init");
    Init.enable().expect("failed to enable init");

    // quit
    let quit_fn: FnQuit = transmute(get_base() + 0x20D810);
    Quit.initialize(quit_fn, quit).expect("failed to hook quit");
    Quit.enable().expect("failed to enable quit")
}

pub unsafe fn disable_hooks() {
    let _ = unsafe { PushButton.disable() };
    let _ = unsafe { ReleaseButton.disable() };
    let _ = unsafe { Init.disable() };
    let _ = unsafe { Quit.disable() };
}
