use crate::BOT;
use geometrydash::{get_base, patch_mem, PlayLayer, PlayerObject, Ptr};
use retour::static_detour;

// pushButton/releaseButton methods that take [PlayerObject].

type FnPushButton = unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;
type FnReleaseButton = unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;

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
    // log::info!("pushbutton: {button}, ");
    unsafe { BOT.on_action(true, BOT.is_player2_obj(player)) };
    res
}

fn release_button(player: PlayerObject, _edx: Ptr, button: i32) -> bool {
    let res = unsafe { ReleaseButton.call(player, 0, button) };
    // log::info!("releasebutton: {button}");

    // what? FIXME
    if button != 2011438332 {
        unsafe { BOT.on_action(false, BOT.is_player2_obj(player)) };
    }
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

macro_rules! patch {
    ($addr:expr, $data:expr) => {
        let len = $data.len();
        let _ = patch_mem($addr, $data)
            .map_err(|e| log::error!("failed to write {len} bytes at {:#x}: {e}", $addr));
    };
}

pub fn anticheat_bypass() {
    log::trace!("activating anticheat bypass");
    patch!(get_base() + 0x202aaa, &[0xeb, 0x2e]);
    patch!(get_base() + 0x15fc2e, &[0xeb]);
    patch!(get_base() + 0x1fd557, &[0xeb, 0x0c]);
    patch!(
        get_base() + 0x1fd742,
        &[
            0xc7, 0x87, 0xe0, 0x02, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xc7, 0x87, 0xe4, 0x02,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
        ]
    );
    patch!(get_base() + 0x1fd756, &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90]);
    patch!(get_base() + 0x1fd79a, &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90]);
    patch!(get_base() + 0x1fd7af, &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90]);
    patch!(get_base() + 0x20d3b3, &[0x90, 0x90, 0x90, 0x90, 0x90]);
    patch!(get_base() + 0x1ff7a2, &[0x90, 0x90]);
    patch!(get_base() + 0x18b2b4, &[0xb0, 0x01]);
    patch!(get_base() + 0x20c4e6, &[0xe9, 0xd7, 0x00, 0x00, 0x00, 0x90]);
}

pub unsafe fn init_hooks() {
    use std::mem::transmute;
    anticheat_bypass();

    // pushbutton
    let push_button_fn: FnPushButton = transmute(get_base() + 0x1F4E40);
    PushButton
        .initialize(push_button_fn, push_button)
        .expect("failed to hook PushButton");
    PushButton
        .enable()
        .expect("failed to enable PushButton hook");

    // releasebutton (same type as FnPushButton)
    let release_button_fn: FnReleaseButton = transmute(get_base() + 0x1F4F70);
    ReleaseButton
        .initialize(release_button_fn, release_button)
        .expect("failed to hook ReleaseButton");
    ReleaseButton
        .enable()
        .expect("failed to enable ReleaseButton hook");

    // init
    let init_fn: FnInit = transmute(get_base() + 0x01FB780);
    Init.initialize(init_fn, init).expect("failed to hook Init");
    Init.enable().expect("failed to enable Init hook");

    // quit
    let quit_fn: FnQuit = transmute(get_base() + 0x20D810);
    Quit.initialize(quit_fn, quit).expect("failed to hook Quit");
    Quit.enable().expect("failed to enable Quit hook")
}

pub unsafe fn disable_hooks() {
    log::debug!("disabling hooks");
    let _ = unsafe { PushButton.disable() }
        .map_err(|e| log::error!("failed to disable PushButton hook: {e}"));
    let _ = unsafe { ReleaseButton.disable() }
        .map_err(|e| log::error!("failed to disable ReleaseButton hook: {e}"));
    let _ = unsafe { Init.disable() }.map_err(|e| log::error!("failed to disable Init hook: {e}"));
    let _ = unsafe { Quit.disable() }.map_err(|e| log::error!("failed to disable Quit hook: {e}"));
}
