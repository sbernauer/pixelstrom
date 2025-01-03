use anyhow::Context;
use futures::SinkExt;
use nom::{branch::alt, bytes::complete::tag, combinator::map, IResult};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LinesCodec};

const HELP_TEXT: &str = "Help text here :)";

#[derive(Debug)]
pub enum Request {
    Help,
    Size,
}

#[derive(Debug)]

pub enum Response {
    Help,
    Size { width: u32, height: u32 },
}

pub fn parse_request(i: &str) -> IResult<&str, Request> {
    alt((parse_help, parse_size))(i)
}

fn parse_help(i: &str) -> IResult<&str, Request> {
    map(tag("HELP"), |_| Request::Help)(i)
}

fn parse_size(i: &str) -> IResult<&str, Request> {
    map(tag("SIZE"), |_| Request::Size)(i)
}

// fn help_parser(i: &str) -> IResult<&str, &str> {
//     tag("abcd")(i) // will consume bytes if the input begins with "abcd"
// }

impl Response {
    pub async fn send_response(
        &self,
        framed: &mut Framed<TcpStream, LinesCodec>,
    ) -> anyhow::Result<()> {
        match self {
            Response::Help => framed.send(HELP_TEXT).await,
            Response::Size { width, height } => framed.send(format!("SIZE {width} {height}")).await,
        }
        .context("Failed to send response to client")?;

        Ok(())
    }
}
