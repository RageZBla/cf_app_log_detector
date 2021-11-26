# cf-app-log-detector
CLI tool to detect Cloud Foundry application logs

## Usage

```
$ cf-app-log-detector app.log
cf-app-log-detector 0.1.0
Olivier Lechevalier <olivier.lechevalier@gmail.com>
Try to detect log outputted by CF cli

USAGE:
    cf-app-log-detector-darwin [FLAGS] [OPTIONS] [LOG]

FLAGS:
    -d, --debug             Enable debugging
    -h, --help              Prints help information
        --one-line-match    Consider the file to be CF app log if a single line matches expected format
    -V, --version           Prints version information

OPTIONS:
    -p, --percentage-matching <PERCENTAGE_MATCHING>
            Percentage of line matching expected format for the file to be considered an application log [default: 90]


ARGS:
    <LOG>    Log file
```

exit codes:

- `0` log file is a cf application log
- `1` log file does not look like an cf application log

## License

This software is release under [MIT License](LICENSE).
