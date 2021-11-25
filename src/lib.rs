use chrono::prelude::*;
use nom::*;

// https://docs.cloudfoundry.org/devguide/deploy-apps/streaming-logs.html#format
#[derive(Debug, PartialEq, PartialOrd)]
pub enum Component {
    API,
    STAGING,
    ROUTER,
    LOGGREGATOR,
    APPLICATION,
    SSH,
    CELL,
    INVALID,
}

#[derive(Debug)]
pub enum ComponentInfoValid {
    Valid(ComponentInfo),
    Invalid(String),
}

#[derive(Debug)]
pub struct ComponentInfo {
    pub name: Component,
    pub index: u32,
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum Channel {
    STDOUT,
    STDERR,
    INVALID,
}

#[derive(Debug)]
pub enum ChannelValid {
    Valid(Channel),
    Invalid(String),
}

#[derive(Debug)]
pub struct CfAppLogEntry<'a> {
    pub timestamp: DateTime<FixedOffset>,
    pub component: ComponentInfoValid,
    pub channel: ChannelValid,
    pub message: Option<&'a str>,
}

named!(parse_date <&str, DateTime<FixedOffset>>,
    map_res!(
        take_until!(" "),
        |s| {
            DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f%z")
        }
    )
);

named!(parse_component_name <&str, Component>,
    alt!(
        tag!("APP") => { |_| Component::APPLICATION } |
        tag!("API") => { |_| Component::API } |
        tag!("STG") => { |_| Component::STAGING } |
        tag!("RTR") => { |_| Component::ROUTER } |
        tag!("LGR") => { |_| Component::LOGGREGATOR } |
        tag!("SSH") => { |_| Component::SSH } |
        tag!("CELL") => { |_| Component::CELL }
    )
);

named!(parse_component <&str, ComponentInfoValid>,
    alt!(
        delimited!(
            tag!("["),
            do_parse!(
                name: parse_component_name >>
                tag!("/") >>
                index: flat_map!(take_until!("]"), parse_to!(u32)) >>
                (ComponentInfoValid::Valid(ComponentInfo {name, index}))
            ),
            tag!("]")
        ) |
        delimited!(
            tag!("["),
            do_parse!(
                name: parse_component_name >>
                tag!("/") >>
                opt!(take_until_and_consume!("/")) >>
                opt!(take_until_and_consume!("/")) >>
                index: flat_map!(take_until!("]"), parse_to!(u32)) >>
                (ComponentInfoValid::Valid(ComponentInfo {name, index}))
            ),
            tag!("]")
        ) |
        delimited!(
            tag!("["),
            do_parse!(
                p: take_until!("]") >>
                (ComponentInfoValid::Invalid(p.to_string()))
            ),
            tag!("]")
        )
    )
);

named!(parse_channel <&str, ChannelValid>,
    alt!(
        tag!("OUT") => { |_| ChannelValid::Valid(Channel::STDOUT) } |
        tag!("ERR") => { |_| ChannelValid::Valid(Channel::STDERR) }
    )
);

// Rust seems to be unable to see that function in used in parse_cf_app_log
#[allow(dead_code)]
fn parse_message(input: &str) -> IResult<&str, Option<&str>> {
    if input.len() > 0 {
        return IResult::Ok(("", Some(input)));
    } else {
        return IResult::Ok(("", None));
    }
}

named!(pub parse_cf_app_log <&str, CfAppLogEntry>,
    do_parse!(
        many0!(tag!(" ")) >>
        timestamp: parse_date >>
        tag!(" ") >>
        component: parse_component >>
        tag!(" ") >>
        channel: parse_channel >>
        alt!(not!(complete!(non_empty)) => {|_tag| ""} | tag!(" ")) >>
        message: parse_message >>
        ({
            CfAppLogEntry {
                timestamp,
                component,
                channel,
                message,
            }
        })
    )
);

#[cfg(test)]
mod tests {
    use super::*;
    use nom::Context::Code;
    use nom::Err::Error;
    use nom::ErrorKind::MapRes;

    #[test]
    fn test_parse_date() {
        let expected = FixedOffset::east(9 * 3600)
            .ymd(2021, 9, 28)
            .and_hms_milli(11, 58, 42, 730);

        assert_eq!(
            parse_date("2021-09-28T11:58:42.73+0900 "),
            Ok((" ", expected))
        );

        assert_eq!(
            parse_date("[10/Oct/2000:13:55:36] "),
            Err(Error(Code("[10/Oct/2000:13:55:36] ", MapRes)))
        );
    }

    #[test]
    fn test_component_name() {
        assert_eq!(parse_component_name("API"), Ok(("", Component::API)));
        assert_eq!(
            parse_component_name("APP"),
            Ok(("", Component::APPLICATION))
        );
        assert_eq!(parse_component_name("CELL"), Ok(("", Component::CELL)));
        assert_eq!(
            parse_component_name("LGR"),
            Ok(("", Component::LOGGREGATOR))
        );
        assert_eq!(parse_component_name("RTR"), Ok(("", Component::ROUTER)));
        assert_eq!(parse_component_name("SSH"), Ok(("", Component::SSH)));
        assert_eq!(parse_component_name("STG"), Ok(("", Component::STAGING)));
    }

    #[test]
    fn test_component_api() {
        let res = parse_component("[API/0]");
        println!("{:#?}\n", res);
        assert!(res.is_ok(), "res: {:#?}", res);

        let res = res.unwrap();
        assert_eq!(res.0, "");
        match res.1 {
            ComponentInfoValid::Valid(component_info) => {
                assert_eq!(component_info.name, Component::API);
                assert_eq!(component_info.index, 0);
            }
            ComponentInfoValid::Invalid(str) => panic!("Invalid component [{}]", str),
        }
    }

    #[test]
    fn test_component_app() {
        let res = parse_component("[APP/PROC/WEB/0]");
        assert!(res.is_ok(), "res: {:#?}", res);

        let res = res.unwrap();
        assert_eq!(res.0, "");
        match res.1 {
            ComponentInfoValid::Valid(component_info) => {
                assert_eq!(component_info.name, Component::APPLICATION);
                assert_eq!(component_info.index, 0);
            }
            ComponentInfoValid::Invalid(str) => panic!("Invalid component [{}]", str),
        }
    }

    #[test]
    fn test_component_unknown_component() {
        let res = parse_component("[FOO/0]");
        assert!(res.is_ok(), "res: {:#?}", res);

        let res = res.unwrap();
        assert_eq!(res.0, "");
        match res.1 {
            ComponentInfoValid::Valid(_) => panic!("should be Invalid"),
            ComponentInfoValid::Invalid(left) => {
                assert_eq!(left, "FOO/0");
            }
        }
    }

    #[test]
    fn test_channel_stdout() {
        let res = parse_channel("OUT");
        assert!(res.is_ok(), "res: {:#?}", res);

        let res = res.unwrap();
        assert_eq!(res.0, "");
        match res.1 {
            ChannelValid::Valid(channel) => {
                assert_eq!(channel, Channel::STDOUT);
            }
            ChannelValid::Invalid(_) => panic!("should be valid"),
        }
    }

    #[test]
    fn test_channel_stderr() {
        let res = parse_channel("ERR");
        assert!(res.is_ok(), "res: {:#?}", res);

        let res = res.unwrap();
        assert_eq!(res.0, "");
        match res.1 {
            ChannelValid::Valid(channel) => {
                assert_eq!(channel, Channel::STDERR);
            }
            ChannelValid::Invalid(_) => panic!("should be valid"),
        }
    }

    #[test]
    fn test_parse_cf_app_log() {
        let entry = parse_cf_app_log(
            r#"2021-09-28T17:00:09.36+0900 [APP/PROC/WEB/0] OUT 2021-09-28 08:00:09.361 DEBUG [,6152cb8077136e53942078a29eb7d0d8,942078a29eb7d0d8] 15 --- [   scheduling-1] i.s.l.r.s.ReminderEmailSchedulerImpl     : result ===> false"#,
        );
        assert!(entry.is_ok(), "res: {:#?}", entry);

        let entry = entry.unwrap().1;
        assert_eq!(
            entry.timestamp,
            FixedOffset::east(9 * 3600)
                .ymd(2021, 9, 28)
                .and_hms_milli(17, 0, 09, 360)
        );
        match entry.component {
            ComponentInfoValid::Valid(comp) => {
                assert_eq!(comp.name, Component::APPLICATION);
                assert_eq!(comp.index, 0);
            }
            ComponentInfoValid::Invalid(_) => panic!("should be valid"),
        }
        match entry.channel {
            ChannelValid::Valid(chan) => {
                assert_eq!(chan, Channel::STDOUT);
            }
            ChannelValid::Invalid(_) => panic!("should be valid"),
        }
        assert_eq!(
			entry.message,
			Some("2021-09-28 08:00:09.361 DEBUG [,6152cb8077136e53942078a29eb7d0d8,942078a29eb7d0d8] 15 --- [   scheduling-1] i.s.l.r.s.ReminderEmailSchedulerImpl     : result ===> false")
		);
    }

    #[test]
    fn test_parse_cf_app_log_no_message() {
        let entry = parse_cf_app_log(r#"2021-09-28T17:00:09.36+0900 [RTR/0] OUT"#);
        assert!(entry.is_ok(), "res: {:#?}", entry);

        let entry = entry.unwrap().1;
        assert_eq!(
            entry.timestamp,
            FixedOffset::east(9 * 3600)
                .ymd(2021, 9, 28)
                .and_hms_milli(17, 0, 09, 360)
        );
        match entry.component {
            ComponentInfoValid::Valid(comp) => {
                assert_eq!(comp.name, Component::ROUTER);
                assert_eq!(comp.index, 0);
            }
            ComponentInfoValid::Invalid(_) => panic!("should be valid"),
        }
        match entry.channel {
            ChannelValid::Valid(chan) => {
                assert_eq!(chan, Channel::STDOUT);
            }
            ChannelValid::Invalid(_) => panic!("should be valid"),
        }
        assert_eq!(entry.message, None);
    }

    #[test]
    fn test_parse_cf_app_log_with_space_prefix() {
        let entry = parse_cf_app_log(
            r#"     2021-09-28T17:00:09.36+0900 [APP/PROC/WEB/0] OUT 2021-09-28 08:00:09.361 DEBUG [,6152cb8077136e53942078a29eb7d0d8,942078a29eb7d0d8] 15 --- [   scheduling-1] i.s.l.r.s.ReminderEmailSchedulerImpl     : result ===> false"#,
        );
        assert!(entry.is_ok(), "res: {:#?}", entry);

        let entry = entry.unwrap().1;
        assert_eq!(
            entry.timestamp,
            FixedOffset::east(9 * 3600)
                .ymd(2021, 9, 28)
                .and_hms_milli(17, 0, 09, 360)
        );
        match entry.component {
            ComponentInfoValid::Valid(comp) => {
                assert_eq!(comp.name, Component::APPLICATION);
                assert_eq!(comp.index, 0);
            }
            ComponentInfoValid::Invalid(_) => panic!("should be valid"),
        }
        match entry.channel {
            ChannelValid::Valid(chan) => {
                assert_eq!(chan, Channel::STDOUT);
            }
            ChannelValid::Invalid(_) => panic!("should be valid"),
        }
        assert_eq!(
			entry.message,
			Some("2021-09-28 08:00:09.361 DEBUG [,6152cb8077136e53942078a29eb7d0d8,942078a29eb7d0d8] 15 --- [   scheduling-1] i.s.l.r.s.ReminderEmailSchedulerImpl     : result ===> false")
		);
    }
}
