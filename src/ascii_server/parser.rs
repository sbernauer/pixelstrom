use core::str;
use std::time::Duration;

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while_m_n},
    character::complete::{alphanumeric1, char},
    combinator::{map, map_res},
    sequence::{preceded, separated_pair},
    IResult, Parser,
};

// FIXME: This potentially leaks the password from the `Login` request.
// Use something like educe or derive-more to skip this field
#[derive(Debug)]
pub enum Request<'a> {
    Help,
    Size,
    Login {
        username: &'a str,
        password: &'a str,
    },
    GetPixel {
        x: u16,
        y: u16,
    },
    SetPixel {
        x: u16,
        y: u16,
        rgba: u32,
    },
    Done,
}

#[derive(Debug)]

pub enum Response {
    Help,
    Size {
        width: u16,
        height: u16,
    },
    LoginNeeded,
    LoginSucceeded,
    LoginFailed,
    AlreadyLoggedIn,
    GetPixel {
        x: u16,
        y: u16,
        rgba: u32,
    },
    Start {
        max_pixels_per_slot: u32,
        slot_duration: Duration,
    },
    Done {
        num_pixels: u32,
    },
    NotYourSlot,
    QuotaExceeded {
        max_pixels_per_slot: u32,
    },
    SlotNotClosedInTime {
        slot_duration: Duration,
    },
}

pub fn parse_request(i: &str) -> IResult<&str, Request> {
    // Trying to sort descending by number of occurrences for performance reasons
    alt((
        parse_get_or_set_pixel,
        parse_done,
        parse_size,
        parse_login,
        parse_help,
    ))
    .parse(i)
}

fn parse_help(i: &str) -> IResult<&str, Request> {
    map(tag("HELP"), |_| Request::Help).parse(i)
}

fn parse_size(i: &str) -> IResult<&str, Request> {
    map(tag("SIZE"), |_| Request::Size).parse(i)
}

fn parse_done(i: &str) -> IResult<&str, Request> {
    map(tag("DONE"), |_| Request::Done).parse(i)
}

fn parse_login(i: &str) -> IResult<&str, Request> {
    let (i, (username, password)) = preceded(
        tag("LOGIN "),
        separated_pair(alphanumeric1, char(' '), alphanumeric1),
    )
    .parse(i)?;

    Ok((i, Request::Login { username, password }))
}

fn parse_get_or_set_pixel(i: &str) -> IResult<&str, Request> {
    let (i, (x, y)) = preceded(
        tag("PX "),
        separated_pair(
            nom::character::complete::u16,
            char(' '),
            nom::character::complete::u16,
        ),
    )
    .parse(i)?;

    // Read request, as there are no following bytes
    if i.is_empty() {
        return Ok((i, Request::GetPixel { x, y }));
    }

    // As there are bytes left, this needs to be a SetPixel request
    let (i, rgba) = preceded(char(' '), ascii_hex_u32).parse(i)?;

    Ok((i, Request::SetPixel { x, y, rgba }))
}

fn ascii_hex_u32(i: &str) -> IResult<&str, u32> {
    map_res(
        take_while_m_n(6, 6, |c: char| c.is_ascii_hexdigit()),
        |hex: &str| u32::from_str_radix(hex, 16),
    )
    .parse(i)
}
