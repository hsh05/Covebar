//! Registers the dock as a Windows AppBar so maximized windows don't cover it.

use crate::settings::Edge;
use crate::win;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Shell::{
    SHAppBarMessage, ABE_BOTTOM, ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS,
    APPBARDATA,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, WM_APP,
};

pub const CALLBACK_MSG: u32 = WM_APP + 0x42;

fn data(h: HWND) -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: h,
        uCallbackMessage: CALLBACK_MSG,
        ..Default::default()
    }
}

pub fn register(h: HWND) {
    let mut abd = data(h);
    unsafe { SHAppBarMessage(ABM_NEW, &mut abd) };
}

pub fn remove(h: HWND) {
    let mut abd = data(h);
    unsafe { SHAppBarMessage(ABM_REMOVE, &mut abd) };
}

/// Reserve `reserve` physical pixels along `edge` of the primary monitor (none: don't reserve
/// anything, windows may go under the dock), then put the `thickness`-tall window on that edge.
pub fn place(h: HWND, edge: Edge, reserve: i32, thickness: i32) -> RECT {
    let mon = win::monitor_rect(win::primary_monitor());
    let mut rc = if reserve > 0 {
        register(h);
        reserve_space(h, edge, reserve, mon)
    } else {
        remove(h);
        mon
    };
    match edge {
        Edge::Top => rc.bottom = rc.top + thickness,
        Edge::Bottom => rc.top = rc.bottom - thickness,
    }
    unsafe {
        let _ = SetWindowPos(
            h,
            Some(HWND_TOPMOST),
            rc.left,
            rc.top,
            rc.right - rc.left,
            rc.bottom - rc.top,
            SWP_NOACTIVATE,
        );
    }
    rc
}

fn reserve_space(h: HWND, edge: Edge, thickness: i32, mon: RECT) -> RECT {
    let mut abd = data(h);
    abd.uEdge = match edge {
        Edge::Top => ABE_TOP,
        Edge::Bottom => ABE_BOTTOM,
    };
    abd.rc = mon;
    let fit = |rc: &mut RECT| match edge {
        Edge::Top => rc.bottom = rc.top + thickness,
        Edge::Bottom => rc.top = rc.bottom - thickness,
    };
    fit(&mut abd.rc);
    unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut abd) };
    fit(&mut abd.rc);
    unsafe { SHAppBarMessage(ABM_SETPOS, &mut abd) };
    abd.rc
}

/// Only the visible bar takes clicks; the rest of the window lets them through to the windows
/// below. `rect` is in physical px relative to the window; None makes the whole window solid.
pub fn set_hit_region(h: HWND, rect: Option<RECT>) {
    use windows::Win32::Graphics::Gdi::{CreateRectRgn, SetWindowRgn, HRGN};
    unsafe {
        let rgn = match rect {
            Some(r) => CreateRectRgn(r.left, r.top, r.right, r.bottom),
            None => HRGN::default(),
        };
        // The window owns the region from here on.
        let _ = SetWindowRgn(h, Some(rgn), true);
    }
}
