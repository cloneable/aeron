use aeron_client_sys::{
    aeron_async_add_publication, aeron_async_add_publication_get_registration_id,
    aeron_async_add_publication_poll, aeron_context_init, aeron_init,
    aeron_publication_channel_status, aeron_publication_is_connected, aeron_start,
};
use core::ptr;
use std::{hint::spin_loop, process::exit};

fn main() {
    let channel = b"aeron:ipc\0" as *const u8 as *const i8;
    let stream_id = 1001;

    let mut context = core::ptr::null_mut();
    handle_error(unsafe { aeron_context_init(&mut context) });
    println!("aeron_context_init");

    let mut client = core::ptr::null_mut();
    handle_error(unsafe { aeron_init(&mut client, context) });
    println!("aeron_init");
    handle_error(unsafe { aeron_start(client) });
    println!("aeron_start");

    let mut add_publication = ptr::null_mut();
    handle_error(unsafe {
        aeron_async_add_publication(&mut add_publication, client, channel, stream_id)
    });
    println!("aeron_async_add_publication");

    let reg_id = unsafe { aeron_async_add_publication_get_registration_id(add_publication) };
    if reg_id < 0 {
        handle_error(reg_id as i32);
    }
    println!("aeron_async_add_publication_get_registration_id: {reg_id}");

    let mut publication = ptr::null_mut();
    loop {
        match unsafe { aeron_async_add_publication_poll(&mut publication, add_publication) } {
            0 => {}
            1 => break,
            e => handle_error(e),
        }
        spin_loop();
    }
    println!("aeron_async_add_publication_poll");

    match unsafe { aeron_publication_channel_status(publication) } {
        -1 => println!("aeron_publication_channel_status: active"),
        1 => println!("aeron_publication_channel_status: active"),
        e => handle_error(e as i32),
    }

    if unsafe { aeron_publication_is_connected(publication) } {
        println!("aeron_publication_is_connected: subscriber found");
    } else {
        println!("aeron_publication_is_connected: no active subscriber");
    }

    // aeron_publication_close(publication, ptr::null_mut(), ptr::null_mut());
    // aeron_close(client);
    // aeron_context_close(context);
}

fn handle_error(code: i32) {
    if code < 0 {
        println!("ERR{code}");
        exit(1);
    }
}
