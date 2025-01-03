use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alphanumeric1, char},
    combinator::map,
    sequence::{preceded, separated_pair},
    IResult,
};

#[derive(Debug)]
// FIXME: This potentially leaks the password from the `Login` request.
// Use something like educe or derive-more to skip this field
pub enum Request<'a> {
    Help,
    Size,
    Login {
        username: &'a str,
        password: &'a str,
    },
}

#[derive(Debug)]

pub enum Response {
    Help,
    Size { width: u32, height: u32 },
    LoginNeeded,
    LoginSucceeded,
    LoginFailed,
}

pub fn parse_request(i: &str) -> IResult<&str, Request> {
    // Trying to sort descending by number of occurrences for performance reasons
    alt((parse_size, parse_login, parse_help))(i)
}

fn parse_help(i: &str) -> IResult<&str, Request> {
    map(tag("HELP"), |_| Request::Help)(i)
}

fn parse_size(i: &str) -> IResult<&str, Request> {
    map(tag("SIZE"), |_| Request::Size)(i)
}

fn parse_login(i: &str) -> IResult<&str, Request> {
    let (i, (username, password)) = preceded(
        tag("LOGIN "),
        separated_pair(alphanumeric1, char(' '), alphanumeric1),
    )(i)?;

    Ok((i, Request::Login { username, password }))
}
