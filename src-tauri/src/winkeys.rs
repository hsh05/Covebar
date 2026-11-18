//! Win+1 … Win+9 and Win+0 open the matching app on the bar, like they do on the Windows
//! taskbar. Explorer keeps those keys registered even while its taskbar is hidden, so they
//! are caught with a low-level keyboard hook. Only plain Win+number is taken: with Shift,
//! Ctrl or Alt held the keys go through to Windows as usual.
//!
//! The hook runs for every key press on the system, so it only compares numbers and hands
//! the real work to another thread. It never records or logs what is typed.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::OnceLock;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, HC_ACTION,
    KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

static ENABLED: AtomicBool = AtomicBool::new(false);
/// Bit 1: left Win is down, bit 2: right Win.
static WIN: AtomicU8 = AtomicU8::new(0);
/// The number key we swallowed, so its repeats and release are swallowed too.
static TAKEN: AtomicU32 = AtomicU32::new(0);
static TX: OnceLock<Sender<u32>> = OnceLock::new();

/// An unassigned key, sent while Win is held so letting go of Win doesn't open Start.
const VK_MASK: u16 = 0xE8;

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
}

pub fn start(on: bool) {
    set_enabled(on);
    let (tx, rx) = channel::<u32>();
    let _ = TX.set(tx);
    std::thread::spawn(move || {
        for vk in rx {
            mask_win_release();
            let n = vk - 0x30;
            crate::apps::open_nth(if n == 0 { 9 } else { n as usize - 1 });
        }
    });
    std::thread::spawn(|| unsafe {
        let module = GetModuleHandleW(None).ok().map(|m| m.into());
        if let Err(e) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), module, 0) {
            crate::dlog!("keyboard hook for Win+number failed: {e}");
            return;
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

fn held(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0 }
}

unsafe extern "system" fn hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let k = &*(lp.0 as *const KBDLLHOOKSTRUCT);
        let down = matches!(wp.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        let vk = k.vkCode;
        if vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32 {
            let bit = if vk == VK_LWIN.0 as u32 { 1 } else { 2 };
            if down {
                WIN.fetch_or(bit, Ordering::SeqCst);
            } else {
                WIN.fetch_and(!bit, Ordering::SeqCst);
            }
        } else if (0x30..=0x39).contains(&vk) && k.flags.0 & LLKHF_INJECTED.0 == 0 {
            if TAKEN.load(Ordering::SeqCst) == vk {
                if !down {
                    TAKEN.store(0, Ordering::SeqCst);
                }
                return LRESULT(1);
            }
            if down
                && ENABLED.load(Ordering::SeqCst)
                && WIN.load(Ordering::SeqCst) != 0
                && !held(VK_SHIFT)
                && !held(VK_CONTROL)
                && !held(VK_MENU)
            {
                if let Some(tx) = TX.get() {
                    TAKEN.store(vk, Ordering::SeqCst);
                    let _ = tx.send(vk);
                    return LRESULT(1);
                }
            }
        }
    }
    CallNextHookEx(None, code, wp, lp)
}

fn mask_win_release() {
    let key = |up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(VK_MASK),
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                ..Default::default()
            },
        },
    };
    unsafe {
        SendInput(&[key(false), key(true)], std::mem::size_of::<INPUT>() as i32);
    }
}
