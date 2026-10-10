//! 无窗口命令行：脚本与 AI 代理用它读改 DBC。
//!
//! 入口是 `roxy-dbc.exe dbc <命令> <文件> [选项]`，不建窗口、不弹对话框：
//! 结果走 stdout，错误走 stderr，退出码 0（成功）/ 1（validate 查出错误）/
//! 2（命令没做成：参数、目标不存在、读写失败）。
//!
//! 命令表 `COMMANDS` 同时是参数校验和 `--help` 文本的唯一来源，加选项只改这张表。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use can_dbc::{ByteOrder, ValueType};
use encoding_rs::Encoding;

use crate::editable_dbc::{
    AttrTarget, AttrValue, EditableDbc, EditableMessage, EditableSignal, FrameFormat, Severity,
    attr_names, is_valid_dbc_identifier, next_free_message_id,
};
use crate::file_encoding::{decode_file_bytes, encode_to_bytes};

/// 一个选项：`value` 为 Some 表示需要取值，None 是开关
#[derive(Clone, Copy)]
struct Opt {
    name: &'static str,
    value: Option<&'static str>,
    help: &'static str,
}

#[derive(Clone, Copy)]
struct Command {
    /// 命令行上的位置，如 "comment" 或 "message add"
    path: &'static str,
    summary: &'static str,
    /// 一行语法提示，`--out/--backup` 由 writes 自动追加
    syntax: &'static str,
    options: &'static [Opt],
    /// 会写文件的命令才有 --out / --backup
    writes: bool,
    /// 支持 --json 的只读命令
    json: bool,
}

const OPT_MESSAGE: Opt = Opt {
    name: "message",
    value: Some("<M>"),
    help: "message to act on: name, or ID in decimal / 0x hex",
};
const OPT_SIGNAL: Opt = Opt {
    name: "signal",
    value: Some("<M.S>"),
    help: "signal to act on as Message.Signal (message by name or ID)",
};
const OPT_TEXT: Opt = Opt {
    name: "text",
    value: Some("<s>"),
    help: "comment text to write; pass an empty string to write an empty one",
};
const OPT_CLEAR: Opt = Opt {
    name: "clear",
    value: None,
    help: "remove the comment / attribute instead of setting it",
};
const OPT_ID: Opt = Opt {
    name: "id",
    value: Some("<id>"),
    help: "message ID, decimal or 0x hex (add: any free ID when omitted)",
};
const OPT_NAME: Opt = Opt {
    name: "name",
    value: Some("<n>"),
    help: "DBC identifier: letters, digits and underscore",
};
const OPT_SIZE: Opt = Opt {
    name: "size",
    value: Some("<bytes>"),
    help: "message length in bytes (8 for classic CAN, up to 64 for CAN FD)",
};
const OPT_TRANSMITTER: Opt = Opt {
    name: "transmitter",
    value: Some("<node>"),
    help: "sending node; must already be a node, or Vector__XXX",
};
const OPT_FRAME: Opt = Opt {
    name: "frame",
    value: Some("<format>"),
    help: "standard | extended | standard-fd | extended-fd",
};
const OPT_CYCLE: Opt = Opt {
    name: "cycle",
    value: Some("<ms>"),
    help: "cycle time written to the declared cycle attribute (GenMsgCycleTime, or CycleTime when that is what the file declares)",
};
const OPT_START: Opt = Opt {
    name: "start",
    value: Some("<bit>"),
    help: "start bit",
};
const OPT_BITS: Opt = Opt {
    name: "bits",
    value: Some("<n>"),
    help: "signal length in bits",
};
const OPT_BYTE_ORDER: Opt = Opt {
    name: "byte-order",
    value: Some("<order>"),
    help: "intel | motorola",
};
const OPT_VALUE_TYPE: Opt = Opt {
    name: "type",
    value: Some("<type>"),
    help: "unsigned | signed",
};
const OPT_FACTOR: Opt = Opt {
    name: "factor",
    value: Some("<v>"),
    help: "physical value factor",
};
const OPT_OFFSET: Opt = Opt {
    name: "offset",
    value: Some("<v>"),
    help: "physical value offset",
};
const OPT_MIN: Opt = Opt {
    name: "min",
    value: Some("<v>"),
    help: "minimum physical value",
};
const OPT_MAX: Opt = Opt {
    name: "max",
    value: Some("<v>"),
    help: "maximum physical value",
};
const OPT_UNIT: Opt = Opt {
    name: "unit",
    value: Some("<s>"),
    help: "unit text, e.g. km/h",
};
const OPT_RECEIVERS: Opt = Opt {
    name: "receivers",
    value: Some("<a,b>"),
    help: "comma-separated receiving nodes; each must already be a node",
};
const OPT_VALUES: Opt = Opt {
    name: "values",
    value: Some("<VAL_ text>"),
    help: "value table written the way VAL_ stores it: 0 \"Off\" 1 \"On\"",
};
const OPT_ATTR: Opt = Opt {
    name: "name",
    value: Some("<Attribute>"),
    help: "attribute name, e.g. GenMsgCycleTime",
};
const OPT_VALUE: Opt = Opt {
    name: "value",
    value: Some("<v>"),
    help: "value to write; an INT/HEX declaration takes an integer, a FLOAT one a number, an ENUM one a listed text",
};
const OPT_TO: Opt = Opt {
    name: "to",
    value: Some("<n>"),
    help: "new node name; every sender and receiver reference is updated",
};

const OPT_OUT: Opt = Opt {
    name: "out",
    value: Some("<path>"),
    help: "write the result to <path> instead of overwriting FILE",
};
const OPT_BACKUP: Opt = Opt {
    name: "backup",
    value: None,
    help: "copy FILE to FILE.bak before overwriting it",
};
const OPT_JSON: Opt = Opt {
    name: "json",
    value: None,
    help: "machine-readable JSON instead of text",
};

const COMMANDS: &[Command] = &[
    Command {
        path: "show",
        summary: "print the database: nodes, messages, signals, comments, attributes, value tables",
        syntax: "dbc show FILE [--message <M>] [--json]",
        options: &[OPT_MESSAGE, OPT_JSON],
        writes: false,
        json: true,
    },
    Command {
        path: "comment",
        summary: "add, replace or remove a message or signal comment (CM_)",
        syntax: "dbc comment FILE (--message <M> | --signal <M.S>) (--text <s> | --clear)",
        options: &[OPT_MESSAGE, OPT_SIGNAL, OPT_TEXT, OPT_CLEAR],
        writes: true,
        json: false,
    },
    Command {
        path: "message add",
        summary: "add a message",
        syntax: "dbc message add FILE --name <N> [--id <id>] [--size <bytes>] [--transmitter <node>] [--frame <format>] [--cycle <ms>] [--text <s>]",
        options: &[
            OPT_NAME,
            OPT_ID,
            OPT_SIZE,
            OPT_TRANSMITTER,
            OPT_FRAME,
            OPT_CYCLE,
            OPT_TEXT,
        ],
        writes: true,
        json: false,
    },
    Command {
        path: "message set",
        summary: "change message fields; only the options given change",
        syntax: "dbc message set FILE --message <M> [--id <id>] [--name <n>] [--size <bytes>] [--transmitter <node>] [--frame <format>] [--cycle <ms>] [--text <s> | --clear]",
        options: &[
            OPT_MESSAGE,
            OPT_ID,
            OPT_NAME,
            OPT_SIZE,
            OPT_TRANSMITTER,
            OPT_FRAME,
            OPT_CYCLE,
            OPT_TEXT,
            OPT_CLEAR,
        ],
        writes: true,
        json: false,
    },
    Command {
        path: "message delete",
        summary: "delete a message with its signals",
        syntax: "dbc message delete FILE --message <M>",
        options: &[OPT_MESSAGE],
        writes: true,
        json: false,
    },
    Command {
        path: "signal add",
        summary: "add a signal to a message",
        syntax: "dbc signal add FILE --message <M> --name <S> --start <bit> --bits <n> [--byte-order <order>] [--type <type>] [--factor <v>] [--offset <v>] [--min <v>] [--max <v>] [--unit <s>] [--receivers <a,b>] [--values <VAL_ text>] [--text <s>]",
        options: &[
            OPT_MESSAGE,
            OPT_NAME,
            OPT_START,
            OPT_BITS,
            OPT_BYTE_ORDER,
            OPT_VALUE_TYPE,
            OPT_FACTOR,
            OPT_OFFSET,
            OPT_MIN,
            OPT_MAX,
            OPT_UNIT,
            OPT_RECEIVERS,
            OPT_VALUES,
            OPT_TEXT,
        ],
        writes: true,
        json: false,
    },
    Command {
        path: "signal set",
        summary: "change signal fields; only the options given change. Renaming updates every reference",
        syntax: "dbc signal set FILE --signal <M.S> [--name <n>] [--start <bit>] [--bits <n>] [--byte-order <order>] [--type <type>] [--factor <v>] [--offset <v>] [--min <v>] [--max <v>] [--unit <s>] [--receivers <a,b>] [--values <VAL_ text>] [--text <s> | --clear]",
        options: &[
            OPT_SIGNAL,
            OPT_NAME,
            OPT_START,
            OPT_BITS,
            OPT_BYTE_ORDER,
            OPT_VALUE_TYPE,
            OPT_FACTOR,
            OPT_OFFSET,
            OPT_MIN,
            OPT_MAX,
            OPT_UNIT,
            OPT_RECEIVERS,
            OPT_VALUES,
            OPT_TEXT,
            OPT_CLEAR,
        ],
        writes: true,
        json: false,
    },
    Command {
        path: "signal delete",
        summary: "delete a signal",
        syntax: "dbc signal delete FILE --signal <M.S>",
        options: &[OPT_SIGNAL],
        writes: true,
        json: false,
    },
    Command {
        path: "attribute set",
        summary: "set or remove a message / signal attribute (BA_); an undeclared name gets a declaration wide enough for the value",
        syntax: "dbc attribute set FILE (--message <M> | --signal <M.S>) --name <Attribute> (--value <v> | --clear)",
        options: &[OPT_MESSAGE, OPT_SIGNAL, OPT_ATTR, OPT_VALUE, OPT_CLEAR],
        writes: true,
        json: false,
    },
    Command {
        path: "node add",
        summary: "add network nodes (BU_) by name",
        syntax: "dbc node add FILE --name <N>",
        options: &[OPT_NAME],
        writes: true,
        json: false,
    },
    Command {
        path: "node rename",
        summary: "rename a node and update every transmitter and receiver reference",
        syntax: "dbc node rename FILE --name <N> --to <new>",
        options: &[OPT_NAME, OPT_TO],
        writes: true,
        json: false,
    },
    Command {
        path: "node delete",
        summary: "delete a node; references to it are left as they are and validate reports them",
        syntax: "dbc node delete FILE --name <N>",
        options: &[OPT_NAME],
        writes: true,
        json: false,
    },
    Command {
        path: "validate",
        summary: "run the same checks as Tools > Validate; exit code 1 when any error is found",
        syntax: "dbc validate FILE [--json]",
        options: &[OPT_JSON],
        writes: false,
        json: true,
    },
];

/// 命令行给出的选项：取值选项进 values，开关进 switches
struct Args {
    values: BTreeMap<String, String>,
    switches: Vec<String>,
}

impl Args {
    fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(|s| s.as_str())
    }

    fn flag(&self, name: &str) -> bool {
        self.switches.iter().any(|s| s == name)
    }

    /// 必填取值选项
    fn required(&self, name: &str, command: &str) -> Result<&str, String> {
        self.value(name).ok_or_else(|| {
            format!("--{name} is required (roxy-dbc {command} --help lists the options)")
        })
    }
}

/// 一次调用：命令、目标文件、选项
struct Invocation {
    command: &'static Command,
    file: PathBuf,
    args: Args,
}

pub struct Outcome {
    pub out: String,
    pub err: String,
    pub code: i32,
}

impl Outcome {
    fn ok(out: impl Into<String>) -> Self {
        Self {
            out: out.into(),
            err: String::new(),
            code: 0,
        }
    }

    /// 命令没做成：参数不对、目标找不到、读写失败
    fn fail(message: impl std::fmt::Display) -> Self {
        Self {
            out: String::new(),
            err: format!("error: {message}\n"),
            code: 2,
        }
    }
}

/// 帮助的同几种写法，Windows 上的 `/?` 也算
fn is_help_word(word: &str) -> bool {
    matches!(word, "-h" | "--help" | "help" | "/?" | "-?")
}

/// 第一个参数是 dbc / 帮助写法时走命令行，其余情况留给图形界面
pub fn is_cli_invocation(argv: &[String]) -> bool {
    argv.first().map(|s| s.as_str()) == Some("dbc") || argv.first().is_some_and(|s| is_help_word(s))
}

fn find(path: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.path == path)
}

/// `help <命令>` 后面跟的是命令名就给那条命令的帮助，否则给总帮助
fn help_for(names: &[String]) -> String {
    match find(&names.join(" ")) {
        Some(command) => command_usage(command),
        None => usage(),
    }
}

/// 该命令接受的选项名（含按 writes / json 追加的公共选项）
fn options_of(command: &Command) -> Vec<&'static Opt> {
    let mut out: Vec<&'static Opt> = command.options.iter().collect();
    if command.json {
        out.push(&OPT_JSON);
    }
    if command.writes {
        out.push(&OPT_OUT);
        out.push(&OPT_BACKUP);
    }
    out
}

fn usage() -> String {
    let mut out = String::new();
    out.push_str("roxy-dbc dbc -- edit CAN description files without a window\n\n");
    out.push_str("  roxy-dbc dbc <command> <file.dbc> [options]\n");
    out.push_str("  roxy-dbc dbc <command> --help    options of one command\n");
    out.push_str("  roxy-dbc dbc help <command>      the same, from the help command\n");
    out.push_str("  roxy-dbc [file ...]              open files in the window (a bare start)\n");
    out.push_str("  roxy-dbc --help | -h | /?        this text\n\n");
    out.push_str("commands\n");
    for command in COMMANDS {
        out.push_str(&format!("  {:<34} {}\n", command.path, command.summary));
    }
    out.push_str(
        "
files
  A .dbc is read, edited and written back in the encoding it was found in
  (UTF-8, UTF-8 BOM or GBK), so the result stays readable by CANdb++ and
  Vector tools. Writing replaces the file atomically; comments on nodes
  (CM_ BU_), environment variables (EV_), signal groups (SIG_GROUP_) and
  standalone value tables (VAL_TABLE_) are not modelled, so a file carrying
  them loses those lines -- the command says so on stderr before writing.

exit codes
  0  the command did its job
  1  validate found errors in the file
  2  the command did not run: bad options, unknown target, file error
",
    );
    out
}

fn command_usage(command: &Command) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "roxy-dbc {}\n  {}\n\n",
        command.syntax, command.summary
    ));
    out.push_str("options\n");
    for opt in options_of(command) {
        let head = match opt.value {
            Some(v) => format!("--{} {}", opt.name, v),
            None => format!("--{}", opt.name),
        };
        out.push_str(&format!("  {:<26} {}\n", head, opt.help));
    }
    out.push_str("  -h, --help                 this text\n");
    out
}

/// 解析停下来有两种原因：要帮助文本（退出码 0），或真的是错误
enum ParseError {
    Help(String),
    Error(String),
}

impl From<String> for ParseError {
    fn from(text: String) -> Self {
        ParseError::Error(text)
    }
}

/// 解析 `dbc <命令> <文件> [选项]`；`--help` 时返回帮助文本
fn parse(argv: &[String]) -> Result<Invocation, ParseError> {
    // 帮助写法：`roxy-dbc --help`、`roxy-dbc help <命令>`、`roxy-dbc dbc help <命令>`
    if argv.first().is_some_and(|first| is_help_word(first)) {
        return Err(ParseError::Help(help_for(argv.get(1..).unwrap_or(&[]))));
    }
    let rest = argv.get(1..).map_or(&[][..], |r| r);
    let Some(first) = rest.first() else {
        return Err(ParseError::Error(
            "which command? try: roxy-dbc dbc show <file.dbc>  (roxy-dbc --help lists them)"
                .to_string(),
        ));
    };
    if is_help_word(first) {
        return Err(ParseError::Help(help_for(rest.get(1..).unwrap_or(&[]))));
    }
    let two_words = match rest.get(1) {
        Some(second) => format!("{first} {second}"),
        None => String::new(),
    };
    let (command, consumed) = match find(&two_words) {
        Some(command) => (command, 2),
        None => match find(first) {
            Some(command) => (command, 1),
            None => {
                return Err(ParseError::Error(format!(
                    "unknown command \"{}\". commands: {}",
                    rest.join(" "),
                    COMMANDS
                        .iter()
                        .map(|c| c.path)
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
        },
    };

    let known = options_of(command);
    let mut values = BTreeMap::new();
    let mut switches = Vec::new();
    let mut file: Option<PathBuf> = None;
    let mut i = consumed;
    while i < rest.len() {
        let token = &rest[i];
        if matches!(token.as_str(), "-h" | "--help" | "/?" | "-?") {
            return Err(ParseError::Help(command_usage(command)));
        }
        let Some(raw) = token.strip_prefix("--") else {
            if file.is_none() {
                file = Some(PathBuf::from(token));
                i += 1;
                continue;
            }
            return Err(ParseError::Error(format!(
                "unexpected argument \"{token}\" -- FILE is the only plain argument"
            )));
        };
        let (name, inline) = match raw.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (raw, None),
        };
        let Some(opt) = known.iter().find(|o| o.name == name) else {
            return Err(ParseError::Error(format!(
                "--{name} is not an option of \"dbc {}\". it accepts: {}",
                command.path,
                known
                    .iter()
                    .map(|o| match o.value {
                        Some(v) => format!("--{} {}", o.name, v),
                        None => format!("--{}", o.name),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };
        match opt.value {
            Some(_) => {
                let value = match inline {
                    Some(v) => v,
                    None => {
                        i += 1;
                        rest.get(i)
                            .cloned()
                            .ok_or_else(|| format!("--{name} needs a value"))?
                    }
                };
                values.insert(name.to_string(), value);
            }
            None => switches.push(name.to_string()),
        }
        i += 1;
    }

    let file = file.ok_or_else(|| {
        format!(
            "no file given. usage: roxy-dbc {}",
            command.syntax.replace("FILE", "<file.dbc>")
        )
    })?;
    Ok(Invocation {
        command,
        file,
        args: Args { values, switches },
    })
}

/// 已读入的 DBC 与它的编码，写回时按原编码输出
struct Loaded {
    dbc: EditableDbc,
    path: PathBuf,
    encoding: &'static Encoding,
    had_bom: bool,
    /// 源文本里出现、但本工具不建模因而写不回去的段
    dropped: Vec<String>,
}

fn load(path: &Path) -> Result<Loaded, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let decoded = decode_file_bytes(&bytes);
    let dbc = can_dbc::Dbc::try_from(decoded.text.as_str())
        .map_err(|e| format!("{} is not a readable DBC: {e:?}", path.display()))?;
    Ok(Loaded {
        dbc: EditableDbc::from_dbc(&dbc),
        path: path.to_path_buf(),
        encoding: decoded.encoding,
        had_bom: decoded.had_bom,
        dropped: dropped_sections(&decoded.text),
    })
}

/// 本工具不建模的段：出现就在 stderr 说明会丢，别让人以为往返无损
const DROPPED_SECTIONS: [(&str, &str); 5] = [
    ("CM_ BU_", "node comments"),
    ("CM_ EV_", "environment variable comments"),
    ("EV_ ", "environment variables"),
    ("SIG_GROUP_", "signal groups"),
    ("VAL_TABLE_", "standalone value tables"),
];

fn dropped_sections(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (marker, what) in DROPPED_SECTIONS {
        let count = text
            .lines()
            .filter(|line| line.trim_start().starts_with(marker))
            .count();
        if count > 0 {
            let shown = marker.trim_end();
            found.push(format!(
                "{count} {what} line(s) ({shown}...): this tool does not model them, so they are not written back"
            ));
        }
    }
    found
}

/// 写出：默认原地替换，--out 写别处（新文件一律 UTF-8）。同目录临时文件 + rename，
/// 中途失败不会留下半个 DBC。
fn save(
    loaded: &Loaded,
    dbc: &EditableDbc,
    args: &Args,
    changes: &[String],
) -> Result<Outcome, String> {
    let target = match args.value("out") {
        Some(path) => PathBuf::from(path),
        None => loaded.path.clone(),
    };
    let (encoding, bom) = if target == loaded.path {
        (loaded.encoding, loaded.had_bom)
    } else {
        // 另存为新文件：默认 UTF-8，与图形界面的另存为一致
        (encoding_rs::UTF_8, false)
    };

    let mut report = String::new();
    for change in changes {
        report.push_str(change);
        report.push('\n');
    }
    if changes.is_empty() {
        report.push_str("nothing to change\n");
    }

    let bytes = encode_to_bytes(&dbc.to_dbc_string(), encoding, bom);
    if args.flag("backup") && target == loaded.path {
        let backup = backup_path(&loaded.path);
        std::fs::copy(&loaded.path, &backup)
            .map_err(|e| format!("cannot write {}: {e}", backup.display()))?;
        report.push_str(&format!("backup: {}\n", backup.display()));
    }
    atomic_write(&target, &bytes)?;
    report.push_str(&format!(
        "wrote {} ({}{}, {} messages, {} signals)\n",
        target.display(),
        encoding.name(),
        if bom { " with BOM" } else { "" },
        dbc.messages().len(),
        dbc.messages()
            .iter()
            .map(|m| m.signals().len())
            .sum::<usize>(),
    ));
    Ok(warn_dropped(loaded, report))
}

fn backup_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file.dbc".to_string());
    path.with_file_name(format!("{name}.bak"))
}

fn atomic_write(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = target.parent().filter(|p| !p.as_os_str().is_empty());
    let file_name = target
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out.dbc".to_string());
    let tmp = match dir {
        Some(dir) => dir.join(format!(".{file_name}.tmp")),
        None => PathBuf::from(format!(".{file_name}.tmp")),
    };
    std::fs::write(&tmp, bytes).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, target).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot replace {}: {e}", target.display())
    })
}

/// 把源文件里会丢掉的段提示一句，不改变退出码
fn warn_dropped(loaded: &Loaded, report: String) -> Outcome {
    let mut err = String::new();
    for line in &loaded.dropped {
        err.push_str(&format!("warning: {line}\n"));
    }
    Outcome {
        out: report,
        err,
        code: 0,
    }
}

/// 把 `--cycle` 写到文件已经声明的那个周期属性上；两个都没声明时用 Vector 的名字
fn cycle_attribute(dbc: &EditableDbc) -> &'static str {
    let genmsg = dbc.attribute_definition(attr_names::MSG_CYCLE_TIME, AttrTarget::Message);
    let plain = dbc.attribute_definition("CycleTime", AttrTarget::Message);
    if genmsg.is_none() && plain.is_some() {
        "CycleTime"
    } else {
        attr_names::MSG_CYCLE_TIME
    }
}

/// 按声明类型收敛取值：声明 INT 的写整数、FLOAT 的写浮点、ENUM/STRING 的写文本，
/// 免得 `BA_` 写出去的类型和 `BA_DEF_` 不一致，别的工具读不回
fn coerce(dbc: &EditableDbc, name: &str, target: AttrTarget, value: AttrValue) -> AttrValue {
    use crate::editable_dbc::AttrType;
    let Some(def) = dbc.attribute_definition(name, target) else {
        return value;
    };
    match &def.kind {
        AttrType::Int { .. } | AttrType::Hex { .. } => match &value {
            AttrValue::Int(_) => value,
            AttrValue::Float(v) => AttrValue::Int(*v as i64),
            AttrValue::Text(v) => parse_int(v).map(AttrValue::Int).unwrap_or(value),
        },
        AttrType::Float { .. } => match &value {
            AttrValue::Float(_) => value,
            AttrValue::Int(v) => AttrValue::Float(*v as f64),
            AttrValue::Text(v) => v
                .trim()
                .parse::<f64>()
                .map(AttrValue::Float)
                .unwrap_or(value),
        },
        AttrType::Enum(_) | AttrType::String => AttrValue::Text(text_of(value)),
    }
}

fn text_of(value: AttrValue) -> String {
    match value {
        AttrValue::Text(v) => v,
        other => other.display(),
    }
}

/// 属性取值文本：数字（含 0x）按数字，其余按文本
fn parse_attr_value(raw: &str) -> AttrValue {
    if let Some(v) = parse_int(raw) {
        return AttrValue::Int(v);
    }
    if let Ok(v) = raw.trim().parse::<f64>() {
        return AttrValue::Float(v);
    }
    AttrValue::Text(raw.to_string())
}

/// 十进制或 0x 十六进制的非负整数
fn parse_int(raw: &str) -> Option<i64> {
    let text = raw.trim();
    if let Some(hex) = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .or_else(|| text.strip_prefix("-0x"))
    {
        let negative = text.starts_with('-');
        let value = u64::from_str_radix(hex, 16).ok()?;
        let value = i64::try_from(value).ok()?;
        return Some(if negative { -value } else { value });
    }
    text.parse::<i64>().ok()
}

fn parse_u64(raw: &str, what: &str) -> Result<u64, String> {
    parse_int(raw)
        .filter(|v| *v >= 0)
        .and_then(|v| u64::try_from(v).ok())
        .ok_or_else(|| format!("{what}=\"{raw}\" is not a non-negative number (0x hex accepted)"))
}

fn parse_f64(raw: &str, what: &str) -> Result<f64, String> {
    raw.trim()
        .parse::<f64>()
        .map_err(|_| format!("{what}=\"{raw}\" is not a number"))
}

fn check_identifier(name: &str, value: &str) -> Result<(), String> {
    if is_valid_dbc_identifier(name) {
        Ok(())
    } else {
        Err(format!(
            "{value}=\"{name}\" is not a DBC identifier: letters, digits and underscore only, and it must not start with a digit"
        ))
    }
}

/// 一条命令的落点：只读命令直接给输出，写命令给出改动说明后由 `save` 落盘
enum Step {
    Read(Outcome),
    Write(Result<Vec<String>, String>),
}

/// 执行 `dbc ...`；argv 是去掉程序名的参数
pub fn execute(argv: &[String]) -> Outcome {
    let invocation = match parse(argv) {
        Ok(invocation) => invocation,
        Err(ParseError::Help(text)) => return Outcome::ok(text),
        Err(ParseError::Error(message)) => return Outcome::fail(message),
    };
    let args = &invocation.args;
    let mut loaded = match load(&invocation.file) {
        Ok(loaded) => loaded,
        Err(message) => return Outcome::fail(message),
    };

    let step = match invocation.command.path {
        "show" => Step::Read(show(&loaded, args)),
        "validate" => Step::Read(validate(&loaded, args)),
        "comment" => Step::Write(comment(&mut loaded, args)),
        "message add" => Step::Write(message_add(&mut loaded, args)),
        "message set" => Step::Write(message_set(&mut loaded, args)),
        "message delete" => Step::Write(message_delete(&mut loaded, args)),
        "signal add" => Step::Write(signal_add(&mut loaded, args)),
        "signal set" => Step::Write(signal_set(&mut loaded, args)),
        "signal delete" => Step::Write(signal_delete(&mut loaded, args)),
        "attribute set" => Step::Write(attribute_set(&mut loaded, args)),
        "node add" => Step::Write(node_add(&mut loaded, args)),
        "node rename" => Step::Write(node_rename(&mut loaded, args)),
        "node delete" => Step::Write(node_delete(&mut loaded, args)),
        other => {
            return Outcome::fail(format!(
                "command \"{other}\" is declared but not implemented"
            ));
        }
    };

    match step {
        Step::Read(outcome) => outcome,
        Step::Write(Ok(changes)) => match save(&loaded, &loaded.dbc, args, &changes) {
            Ok(outcome) => outcome,
            Err(message) => Outcome::fail(message),
        },
        Step::Write(Err(message)) => Outcome::fail(message),
    }
}

// ---------------------------------------------------------------- 目标解析

/// `--message` 的取值：报文名，或十进制 / 0x 写法的 ID
fn message_id(dbc: &EditableDbc, spec: &str) -> Result<u32, String> {
    if let Some(id) = parse_int(spec).and_then(|v| u32::try_from(v).ok()) {
        return dbc
            .find_message_index(id)
            .map(|_| id)
            .ok_or_else(|| format!("no message with ID {spec} here. {}", message_list(dbc)));
    }
    dbc.get_message_by_name(spec)
        .map(|m| m.message_id())
        .ok_or_else(|| format!("no message named \"{spec}\" here. {}", message_list(dbc)))
}

/// 名字里带点才当 `Message.Signal` 用；报文名本身不含点，所以按最后一个点切
fn signal_target(dbc: &EditableDbc, spec: &str) -> Result<(u32, String), String> {
    let (message, signal) = spec
        .rsplit_once('.')
        .ok_or_else(|| format!("--signal wants Message.Signal, got \"{spec}\""))?;
    let id = message_id(dbc, message)?;
    let found = dbc
        .get_message(id)
        .and_then(|m| m.signals().iter().find(|s| s.name() == signal).map(|_| ()));
    found.ok_or_else(|| {
        let known = dbc
            .get_message(id)
            .map(|m| {
                let names: Vec<&str> = m.signals().iter().map(|s| s.name()).collect();
                let shown = if names.len() > 12 {
                    format!("{:?} ... ({} signals)", &names[..12], names.len())
                } else {
                    format!("{names:?}")
                };
                format!("signals of {message}: {shown}")
            })
            .unwrap_or_default();
        format!("no signal \"{signal}\" in message \"{message}\" here. {known}")
    })?;
    Ok((id, signal.to_string()))
}

fn message_list(dbc: &EditableDbc) -> String {
    let names: Vec<String> = dbc
        .messages()
        .iter()
        .take(12)
        .map(|m| format!("{} ({})", m.message_name(), format_id(m)))
        .collect();
    let more = dbc.messages().len().saturating_sub(names.len());
    if more > 0 {
        format!("known messages: {}, ... {} more", names.join(", "), more)
    } else {
        format!("known messages: {}", names.join(", "))
    }
}

fn require_node(dbc: &EditableDbc, node: &str, option: &str) -> Result<(), String> {
    if node == "Vector__XXX" {
        return Ok(());
    }
    if dbc.nodes().iter().any(|n| n == node) {
        return Ok(());
    }
    Err(format!(
        "{option}=\"{node}\" is not a node of this file. add it first: roxy-dbc dbc node add <file> --name {node} (nodes here: {})",
        dbc.nodes().join(", ")
    ))
}

fn parse_frame(text: &str) -> Result<FrameFormat, String> {
    match text.trim().to_ascii_lowercase().as_str() {
        "standard" | "std" | "11" => Ok(FrameFormat::Standard),
        "extended" | "ext" | "29" => Ok(FrameFormat::Extended),
        "standard-fd" | "std-fd" | "fd" => Ok(FrameFormat::StandardFd),
        "extended-fd" | "ext-fd" => Ok(FrameFormat::ExtendedFd),
        other => Err(format!(
            "--frame=\"{other}\" is not one of standard, extended, standard-fd, extended-fd"
        )),
    }
}

fn frame_label(format: FrameFormat) -> &'static str {
    match format {
        FrameFormat::Standard => "standard",
        FrameFormat::Extended => "extended",
        FrameFormat::StandardFd => "standard-fd",
        FrameFormat::ExtendedFd => "extended-fd",
    }
}

fn parse_byte_order(text: &str) -> Result<ByteOrder, String> {
    match text.trim().to_ascii_lowercase().as_str() {
        "intel" | "little" | "le" | "1" => Ok(ByteOrder::LittleEndian),
        "motorola" | "big" | "be" | "0" => Ok(ByteOrder::BigEndian),
        other => Err(format!("--byte-order=\"{other}\" is not intel or motorola")),
    }
}

fn byte_order_label(order: &ByteOrder) -> &'static str {
    match order {
        ByteOrder::LittleEndian => "intel",
        ByteOrder::BigEndian => "motorola",
    }
}

fn parse_value_type(text: &str) -> Result<ValueType, String> {
    match text.trim().to_ascii_lowercase().as_str() {
        "unsigned" | "unsigned64" | "+" => Ok(ValueType::Unsigned),
        "signed" | "signed64" | "-" => Ok(ValueType::Signed),
        other => Err(format!("--type=\"{other}\" is not unsigned or signed")),
    }
}

fn value_type_label(kind: &ValueType) -> &'static str {
    match kind {
        ValueType::Unsigned => "unsigned",
        ValueType::Signed => "signed",
    }
}

fn parse_receivers(dbc: &EditableDbc, text: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for part in text.split(',') {
        let node = part.trim();
        if node.is_empty() || node == "Vector__XXX" {
            continue;
        }
        require_node(dbc, node, "--receivers")?;
        if !out.iter().any(|n| n == node) {
            out.push(node.to_string());
        }
    }
    Ok(out)
}

/// 值表按 `VAL_` 的写法解析：`0 "Off" 1 "On"`，引号可省
fn parse_value_table(text: &str) -> Result<Vec<(i64, String)>, String> {
    if text.contains('=') {
        return Err(
            "--values wants the VAL_ spelling: 0 \"Off\" 1 \"On\" (not 0=Off; 1=On)".to_string(),
        );
    }
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => {
                if quoted && !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                quoted = !quoted;
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if quoted {
        return Err("--values has an unclosed quote".to_string());
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if !tokens.len().is_multiple_of(2) {
        return Err(format!(
            "--values wants value/description pairs like 0 \"Off\" 1 \"On\", got {} token(s)",
            tokens.len()
        ));
    }
    let mut out: Vec<(i64, String)> = Vec::new();
    for pair in tokens.chunks(2) {
        let value = parse_int(&pair[0])
            .ok_or_else(|| format!("--values: \"{}\" is not an integer raw value", pair[0]))?;
        if out.iter().any(|(v, _)| *v == value) {
            return Err(format!("--values: raw value {value} appears twice"));
        }
        out.push((value, pair[1].clone()));
    }
    out.sort_by_key(|(v, _)| *v);
    Ok(out)
}

fn format_id(message: &EditableMessage) -> String {
    if message.frame_format().is_extended() {
        format!("0x{:08X}", message.message_id())
    } else {
        format!("0x{:03X}", message.message_id())
    }
}

fn describe_message(dbc: &EditableDbc, spec: &str) -> String {
    match message_id(dbc, spec) {
        Ok(id) => dbc
            .get_message(id)
            .map(|m| format!("{} {}", format_id(m), m.message_name()))
            .unwrap_or_default(),
        Err(_) => spec.to_string(),
    }
}

// ---------------------------------------------------------------- 写命令

fn comment(loaded: &mut Loaded, args: &Args) -> Result<Vec<String>, String> {
    let text = if args.flag("clear") {
        if args.value("text").is_some() {
            return Err("--text and --clear cannot be used together".to_string());
        }
        String::new()
    } else {
        args.required("text", "comment")?.to_string()
    };

    let dbc = &mut loaded.dbc;
    match (args.value("message"), args.value("signal")) {
        (Some(_), Some(_)) => {
            Err("--message and --signal pick one target; use two calls".to_string())
        }
        (None, None) => Err("say what to comment: --message <M> or --signal <M.S>".to_string()),
        (Some(spec), None) => {
            let id = message_id(dbc, spec)?;
            let name = dbc
                .get_message(id)
                .map(|m| m.message_name().to_string())
                .unwrap_or_default();
            dbc.set_message_comment(id, &text);
            Ok(vec![comment_line(
                "BO_",
                &format_id_of(dbc, id),
                &name,
                &text,
            )])
        }
        (None, Some(spec)) => {
            let (id, signal) = signal_target(dbc, spec)?;
            dbc.set_signal_comment(id, &signal, &text);
            Ok(vec![comment_line(
                "SG_",
                &format_id_of(dbc, id),
                &signal,
                &text,
            )])
        }
    }
}

fn format_id_of(dbc: &EditableDbc, id: u32) -> String {
    dbc.get_message(id)
        .map(format_id)
        .unwrap_or_else(|| format!("0x{id:03X}"))
}

fn comment_line(kind: &str, id: &str, name: &str, text: &str) -> String {
    if text.is_empty() {
        format!("comment {kind} {id} {name}: removed")
    } else {
        format!("comment {kind} {id} {name}: \"{text}\"")
    }
}

fn message_add(loaded: &mut Loaded, args: &Args) -> Result<Vec<String>, String> {
    let dbc = &mut loaded.dbc;
    let name = args.required("name", "message add")?.to_string();
    check_identifier(&name, "--name")?;
    if dbc.get_message_by_name(&name).is_some() {
        return Err(format!("a message named \"{name}\" already exists here"));
    }
    let format = match args.value("frame") {
        Some(text) => parse_frame(text)?,
        None => FrameFormat::Standard,
    };
    let size = match args.value("size") {
        Some(text) => parse_u64(text, "--size")?,
        None => 8,
    };
    if size > 8 && !format.is_fd() {
        return Err(format!(
            "--size {size} does not fit a classic CAN frame: add --frame standard-fd or --frame extended-fd"
        ));
    }
    let id = match args.value("id") {
        Some(raw) => new_message_id(dbc, raw, format)?,
        None => next_free_message_id(dbc, 1),
    };
    let transmitter = match args.value("transmitter") {
        Some(node) => {
            require_node(dbc, node, "--transmitter")?;
            node.to_string()
        }
        None => "Vector__XXX".to_string(),
    };
    let comment = args.value("text").unwrap_or_default().to_string();
    dbc.add_message(&EditableMessage::build(
        id,
        format,
        name.clone(),
        size,
        transmitter.clone(),
        Vec::new(),
        comment.clone(),
    ));

    let shown = format_id_of(dbc, id);
    let mut changes = vec![format!(
        "message add: BO_ {shown} {name} size={size} frame={} transmitter={transmitter}",
        frame_label(format)
    )];
    if !comment.is_empty() {
        changes.push(comment_line("BO_", &shown, &name, &comment));
    }
    if let Some(cycle) = args.value("cycle") {
        changes.push(set_cycle(dbc, id, cycle)?);
    }
    Ok(changes)
}

fn new_message_id(dbc: &EditableDbc, raw: &str, format: FrameFormat) -> Result<u32, String> {
    let id = parse_int(raw)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v != 0)
        .ok_or_else(|| format!("--id=\"{raw}\" is not a message ID (decimal or 0x hex)"))?;
    let (limit, kind) = if format.is_extended() {
        (0x1FFF_FFFFu32, "29-bit extended")
    } else {
        (0x7FFu32, "11-bit standard")
    };
    if id > limit {
        return Err(format!(
            "--id {id} does not fit {kind} addressing (highest {limit})"
        ));
    }
    if dbc.find_message_index(id).is_some() {
        return Err(format!("--id {id} (0x{id:03X}) is already used here"));
    }
    Ok(id)
}

fn message_set(loaded: &mut Loaded, args: &Args) -> Result<Vec<String>, String> {
    let spec = args.required("message", "message set")?.to_string();
    let dbc = &mut loaded.dbc;
    let id = message_id(dbc, &spec)?;
    let mut changes: Vec<String> = Vec::new();

    if let Some(raw) = args.value("frame") {
        let format = parse_frame(raw)?;
        let old = dbc
            .get_message(id)
            .map(|m| m.frame_format())
            .unwrap_or(FrameFormat::Standard);
        let size = dbc
            .get_message(id)
            .map(|m| m.message_size())
            .unwrap_or_default();
        if size > 8 && !format.is_fd() {
            return Err(format!(
                "message {spec} is {size} bytes, so it cannot become a {} frame",
                frame_label(format)
            ));
        }
        dbc.set_message_frame_format(id, format);
        changes.push(format!(
            "frame {} -> {}",
            frame_label(old),
            frame_label(format)
        ));
    }
    if let Some(raw) = args.value("size") {
        let size = parse_u64(raw, "--size")?;
        let format = dbc
            .get_message(id)
            .map(|m| m.frame_format())
            .unwrap_or(FrameFormat::Standard);
        if size > 8 && !format.is_fd() {
            return Err(format!(
                "--size {size} does not fit a classic CAN frame: add --frame standard-fd"
            ));
        }
        dbc.set_message_size(id, size);
        changes.push(format!("size -> {size} bytes"));
    }
    if let Some(raw) = args.value("id") {
        let format = dbc
            .get_message(id)
            .map(|m| m.frame_format())
            .unwrap_or(FrameFormat::Standard);
        let new_id = new_message_id(dbc, raw, format)?;
        let old_label = format_id_of(dbc, id);
        dbc.set_message_id(id, new_id);
        changes.push(format!("id {old_label} -> {raw}"));
    }
    if let Some(raw) = args.value("name") {
        check_identifier(raw, "--name")?;
        if let Some(other) = dbc.get_message_by_name(raw)
            && other.message_id() != id
        {
            return Err(format!("a message named \"{raw}\" already exists here"));
        }
        dbc.set_message_name(id, raw);
        changes.push(format!("name -> {raw}"));
    }
    if let Some(raw) = args.value("transmitter") {
        require_node(dbc, raw, "--transmitter")?;
        dbc.set_message_transmitter(id, raw);
        changes.push(format!("transmitter -> {raw}"));
    }
    if let Some(raw) = args.value("cycle") {
        changes.push(set_cycle(dbc, id, raw)?);
    }
    if args.flag("clear") && args.value("text").is_some() {
        return Err("--text and --clear cannot be used together".to_string());
    }
    if let Some(text) = args.value("text") {
        dbc.set_message_comment(id, text);
        let name = dbc
            .get_message(id)
            .map(|m| m.message_name().to_string())
            .unwrap_or_default();
        changes.push(comment_line("BO_", &format_id_of(dbc, id), &name, text));
    } else if args.flag("clear") {
        dbc.set_message_comment(id, "");
        changes.push(format!("comment BO_ {}: removed", format_id_of(dbc, id)));
    }

    if changes.is_empty() {
        return Err(
            "nothing to change: pass one of --id --name --size --transmitter --frame --cycle --text --clear (dbc message set --help)"
                .to_string(),
        );
    }
    changes.insert(0, format!("message set: {}", describe_message(dbc, &spec)));
    Ok(changes)
}

fn set_cycle(dbc: &mut EditableDbc, id: u32, raw: &str) -> Result<String, String> {
    let value = parse_int(raw)
        .filter(|v| *v >= 0)
        .ok_or_else(|| format!("--cycle=\"{raw}\" is not a whole number of milliseconds"))?;
    let name = cycle_attribute(dbc);
    dbc.set_message_attribute(id, name, Some(AttrValue::Int(value)));
    Ok(format!(
        "cycle {name} {} -> {value} ms",
        format_id_of(dbc, id)
    ))
}

fn message_delete(loaded: &mut Loaded, args: &Args) -> Result<Vec<String>, String> {
    let spec = args.required("message", "message delete")?.to_string();
    let dbc = &mut loaded.dbc;
    let id = message_id(dbc, &spec)?;
    let message = dbc
        .get_message(id)
        .ok_or_else(|| format!("no message {spec} here"))?;
    let name = message.message_name().to_string();
    let signals = message.signals_count();
    dbc.delete_message(id);
    Ok(vec![format!(
        "message delete: BO_ {} {name} ({signals} signals removed with it)",
        format_id_of(dbc, id)
    )])
}

fn signal_add(loaded: &mut Loaded, args: &Args) -> Result<Vec<String>, String> {
    let spec = args.required("message", "signal add")?.to_string();
    let dbc = &mut loaded.dbc;
    let id = message_id(dbc, &spec)?;
    let name = args.required("name", "signal add")?.to_string();
    check_identifier(&name, "--name")?;
    if dbc.find_signal_index(id, &name).is_some() {
        return Err(format!(
            "message {spec} already has a signal named \"{name}\""
        ));
    }
    let start = parse_u64(args.required("start", "signal add")?, "--start")?;
    let bits = parse_u64(args.required("bits", "signal add")?, "--bits")?;
    if bits == 0 || bits > 64 {
        return Err(format!("--bits {bits} is outside 1..64"));
    }
    let message_size = dbc
        .get_message(id)
        .map(|m| m.message_size())
        .unwrap_or_default();
    if start >= message_size * 8 {
        return Err(format!(
            "--start {start} is past the end of {spec}, which is {message_size} bytes"
        ));
    }
    let byte_order = match args.value("byte-order") {
        Some(text) => parse_byte_order(text)?,
        None => ByteOrder::LittleEndian,
    };
    let value_type = match args.value("type") {
        Some(text) => parse_value_type(text)?,
        None => ValueType::Unsigned,
    };
    let factor = number(args, "factor", 1.0)?;
    let offset = number(args, "offset", 0.0)?;
    let min = number(args, "min", 0.0)?;
    let max = number(args, "max", 0.0)?;
    if min > max {
        return Err(format!("--min {min} is above --max {max}"));
    }
    let unit = args.value("unit").unwrap_or_default().to_string();
    let receivers = match args.value("receivers") {
        Some(text) => parse_receivers(dbc, text)?,
        None => Vec::new(),
    };
    let values = match args.value("values") {
        Some(text) => parse_value_table(text)?,
        None => Vec::new(),
    };
    let comment = args.value("text").unwrap_or_default().to_string();

    let signal = EditableSignal::build(
        name.clone(),
        start,
        bits,
        byte_order,
        value_type,
        factor,
        offset,
        min,
        max,
        unit.clone(),
        receivers.clone(),
        values.clone(),
        comment.clone(),
    );
    dbc.add_signal(id, &signal);

    let mut changes = vec![format!(
        "signal add: SG_ {name} start={start} bits={bits} order={} type={}",
        byte_order_label(&byte_order),
        value_type_label(&value_type)
    )];
    if !unit.is_empty() {
        changes.push(format!("unit {name} = \"{unit}\""));
    }
    if !receivers.is_empty() {
        changes.push(format!("receivers {name} = {}", receivers.join(", ")));
    }
    if !values.is_empty() {
        changes.push(format!(
            "values {name} = {}",
            values
                .iter()
                .map(|(v, d)| format!("{v} \"{d}\""))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    if !comment.is_empty() {
        changes.push(comment_line("SG_", &format_id_of(dbc, id), &name, &comment));
    }
    Ok(changes)
}

fn number(args: &Args, name: &str, default: f64) -> Result<f64, String> {
    match args.value(name) {
        Some(raw) => parse_f64(raw, &format!("--{name}")),
        None => Ok(default),
    }
}

fn signal_set(loaded: &mut Loaded, args: &Args) -> Result<Vec<String>, String> {
    let spec = args.required("signal", "signal set")?.to_string();
    let dbc = &mut loaded.dbc;
    let (id, signal) = signal_target(dbc, &spec)?;
    let mut changes: Vec<String> = Vec::new();

    if let Some(raw) = args.value("start") {
        let value = parse_u64(raw, "--start")?;
        dbc.set_signal_start_bit(id, &signal, value);
        changes.push(format!("start -> {value}"));
    }
    if let Some(raw) = args.value("bits") {
        let value = parse_u64(raw, "--bits")?;
        if value == 0 || value > 64 {
            return Err(format!("--bits {value} is outside 1..64"));
        }
        dbc.set_signal_size(id, &signal, value);
        changes.push(format!("bits -> {value}"));
    }
    if let Some(raw) = args.value("byte-order") {
        let value = parse_byte_order(raw)?;
        dbc.set_signal_byte_order(id, &signal, value);
        changes.push(format!("byte order -> {}", byte_order_label(&value)));
    }
    if let Some(raw) = args.value("type") {
        let value = parse_value_type(raw)?;
        dbc.set_signal_value_type(id, &signal, value);
        changes.push(format!("type -> {}", value_type_label(&value)));
    }
    for name in ["factor", "offset", "min", "max"] {
        if let Some(raw) = args.value(name) {
            let value = parse_f64(raw, &format!("--{name}"))?;
            match name {
                "factor" => dbc.set_signal_factor(id, &signal, value),
                "offset" => dbc.set_signal_offset(id, &signal, value),
                "min" => dbc.set_signal_min(id, &signal, value),
                _ => dbc.set_signal_max(id, &signal, value),
            }
            changes.push(format!("{name} -> {value}"));
        }
    }
    if let (Some(min), Some(max)) = (args.value("min"), args.value("max"))
        && parse_f64(min, "--min")? > parse_f64(max, "--max")?
    {
        return Err(format!("--min {min} is above --max {max}"));
    }
    if let Some(raw) = args.value("unit") {
        dbc.set_signal_unit(id, &signal, raw);
        changes.push(format!("unit -> \"{raw}\""));
    }
    if let Some(raw) = args.value("receivers") {
        let receivers = parse_receivers(dbc, raw)?;
        dbc.set_signal_receivers(id, &signal, receivers.clone());
        changes.push(format!(
            "receivers -> {}",
            if receivers.is_empty() {
                "Vector__XXX".to_string()
            } else {
                receivers.join(", ")
            }
        ));
    }
    if let Some(raw) = args.value("values") {
        let values = parse_value_table(raw)?;
        dbc.set_signal_value_descriptions(id, &signal, values.clone());
        changes.push(format!(
            "values -> {}",
            if values.is_empty() {
                "removed".to_string()
            } else {
                values
                    .iter()
                    .map(|(v, d)| format!("{v} \"{d}\""))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        ));
    }
    if args.flag("clear") && args.value("text").is_some() {
        return Err("--text and --clear cannot be used together".to_string());
    }
    if let Some(text) = args.value("text") {
        dbc.set_signal_comment(id, &signal, text);
        changes.push(comment_line("SG_", &format_id_of(dbc, id), &signal, text));
    } else if args.flag("clear") {
        dbc.set_signal_comment(id, &signal, "");
        changes.push(format!(
            "comment SG_ {} {signal}: removed",
            format_id_of(dbc, id)
        ));
    }
    if let Some(raw) = args.value("name") {
        check_identifier(raw, "--name")?;
        if dbc.find_signal_index(id, raw).is_some() {
            return Err(format!(
                "message {} already has a signal named \"{raw}\"",
                format_id_of(dbc, id)
            ));
        }
        dbc.set_signal_name(id, &signal, raw);
        changes.push(format!("name {signal} -> {raw}"));
    }

    if changes.is_empty() {
        return Err(
            "nothing to change: pass one of --name --start --bits --byte-order --type --factor --offset --min --max --unit --receivers --values --text --clear (dbc signal set --help)"
                .to_string(),
        );
    }
    changes.insert(0, format!("signal set: {spec}"));
    Ok(changes)
}

fn signal_delete(loaded: &mut Loaded, args: &Args) -> Result<Vec<String>, String> {
    let spec = args.required("signal", "signal delete")?.to_string();
    let dbc = &mut loaded.dbc;
    let (id, signal) = signal_target(dbc, &spec)?;
    dbc.delete_signal(id, &signal);
    Ok(vec![format!(
        "signal delete: SG_ {signal} of {} (its comment and value table go with it)",
        format_id_of(dbc, id)
    )])
}

fn attribute_set(loaded: &mut Loaded, args: &Args) -> Result<Vec<String>, String> {
    let name = args.required("name", "attribute set")?.to_string();
    let raw = if args.flag("clear") {
        if args.value("value").is_some() {
            return Err("--value and --clear cannot be used together".to_string());
        }
        None
    } else {
        Some(args.required("value", "attribute set")?.to_string())
    };

    let dbc = &mut loaded.dbc;
    match (args.value("message"), args.value("signal")) {
        (Some(_), Some(_)) => Err("--message and --signal pick one target; use two calls".to_string()),
        (None, None) => Err("say what carries the attribute: --message <M> or --signal <M.S> (node and network attributes are not edited here)".to_string()),
        (Some(spec), None) => {
            let id = message_id(dbc, spec)?;
            let value = raw
                .map(|v| coerce(dbc, &name, AttrTarget::Message, parse_attr_value(&v)));
            dbc.set_message_attribute(id, &name, value.clone());
            Ok(vec![attribute_line(
                "BO_",
                &format_id_of(dbc, id),
                &name,
                value.as_ref(),
            )])
        }
        (None, Some(spec)) => {
            let (id, signal) = signal_target(dbc, spec)?;
            let value = raw
                .map(|v| coerce(dbc, &name, AttrTarget::Signal, parse_attr_value(&v)));
            dbc.set_signal_attribute(id, &signal, &name, value.clone());
            Ok(vec![attribute_line(
                "SG_",
                &format!("{} {signal}", format_id_of(dbc, id)),
                &name,
                value.as_ref(),
            )])
        }
    }
}

fn attribute_line(kind: &str, target: &str, name: &str, value: Option<&AttrValue>) -> String {
    match value {
        Some(value) => format!("attribute {kind} {target} {name} = \"{}\"", value.display()),
        None => format!("attribute {kind} {target} {name}: removed"),
    }
}

fn node_add(loaded: &mut Loaded, args: &Args) -> Result<Vec<String>, String> {
    let name = args.required("name", "node add")?.to_string();
    check_identifier(&name, "--name")?;
    let dbc = &mut loaded.dbc;
    if dbc.nodes().contains(&name) {
        return Err(format!("node \"{name}\" is already in BU_ here"));
    }
    dbc.add_node(&name);
    Ok(vec![format!("node add: {name}")])
}

fn node_rename(loaded: &mut Loaded, args: &Args) -> Result<Vec<String>, String> {
    let name = args.required("name", "node rename")?.to_string();
    let to = args.required("to", "node rename")?.to_string();
    check_identifier(&to, "--to")?;
    let dbc = &mut loaded.dbc;
    if !dbc.nodes().contains(&name) {
        return Err(format!(
            "no node \"{name}\" here. nodes: {}",
            dbc.nodes().join(", ")
        ));
    }
    if dbc.nodes().contains(&to) {
        return Err(format!("node \"{to}\" is already in BU_ here"));
    }
    dbc.rename_node(&name, &to);
    Ok(vec![format!(
        "node rename: {name} -> {to} (transmitters and receivers updated)"
    )])
}

fn node_delete(loaded: &mut Loaded, args: &Args) -> Result<Vec<String>, String> {
    let name = args.required("name", "node delete")?.to_string();
    let dbc = &mut loaded.dbc;
    if !dbc.nodes().contains(&name) {
        return Err(format!(
            "no node \"{name}\" here. nodes: {}",
            dbc.nodes().join(", ")
        ));
    }
    let used_by = dbc
        .messages()
        .iter()
        .filter(|m| m.transmitter() == name)
        .count();
    dbc.delete_node(&name);
    let mut changes = vec![format!("node delete: {name}")];
    if used_by > 0 {
        changes.push(format!(
            "warning: {used_by} message(s) still name {name} as transmitter -- dbc validate lists them"
        ));
    }
    Ok(changes)
}

// ---------------------------------------------------------------- 读命令

fn show(loaded: &Loaded, args: &Args) -> Outcome {
    let only = match args.value("message") {
        Some(spec) => match message_id(&loaded.dbc, spec) {
            Ok(id) => Some(id),
            Err(message) => return Outcome::fail(message),
        },
        None => None,
    };
    if args.flag("json") {
        Outcome::ok(json_database(loaded, only))
    } else {
        Outcome::ok(text_database(loaded, only))
    }
}

fn validate(loaded: &Loaded, args: &Args) -> Outcome {
    let issues = loaded.dbc.validate();
    let errors = issues
        .iter()
        .filter(|i| matches!(i.severity, Severity::Error))
        .count();
    let warnings = issues.len() - errors;

    let mut out = String::new();
    if args.flag("json") {
        out.push_str(&format!(
            "{{\"file\":{},\"encoding\":\"{}\",\"errors\":{errors},\"warnings\":{warnings},\"issues\":[",
            json_str(&loaded.path.display().to_string()),
            loaded.encoding.name()
        ));
        for (index, issue) in issues.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"severity\":\"{}\",\"message\":{}}}",
                if matches!(issue.severity, Severity::Error) {
                    "error"
                } else {
                    "warning"
                },
                json_str(&issue.message)
            ));
        }
        out.push_str("]}");
    } else {
        out.push_str(&format!(
            "{}: {errors} error{}, {warnings} warning{}\n",
            loaded.path.display(),
            if errors == 1 { "" } else { "s" },
            if warnings == 1 { "" } else { "s" }
        ));
        for issue in &issues {
            let label = if matches!(issue.severity, Severity::Error) {
                "ERROR"
            } else {
                "WARN "
            };
            out.push_str(&format!("  {label} {}\n", issue.message));
        }
        for line in &loaded.dropped {
            out.push_str(&format!("  NOTE  {line}\n"));
        }
    }
    let code = if errors > 0 { 1 } else { 0 };
    Outcome {
        out,
        err: String::new(),
        code,
    }
}

fn text_database(loaded: &Loaded, only: Option<u32>) -> String {
    let dbc = &loaded.dbc;
    let mut out = String::new();
    out.push_str(&format!(
        "file: {} ({})\n",
        loaded.path.display(),
        loaded.encoding.name()
    ));
    out.push_str(&format!("nodes: {}\n", dbc.nodes().join(", ")));
    let messages: Vec<&EditableMessage> = dbc
        .messages()
        .iter()
        .filter(|m| only.is_none_or(|id| m.message_id() == id))
        .collect();
    out.push_str(&format!("messages: {}\n\n", messages.len()));

    for message in messages {
        out.push_str(&format!(
            "BO_ {} {} size={} frame={} transmitter={}\n",
            format_id(message),
            message.message_name(),
            message.message_size(),
            frame_label(message.frame_format()),
            message.transmitter()
        ));
        if !message.comment().is_empty() {
            out.push_str(&format!("    comment: {}\n", message.comment()));
        }
        if let Some(cycle) = dbc.message_cycle_time(message.message_id()) {
            out.push_str(&format!("    cycle: {} ms\n", cycle.display()));
        }
        if !message.attributes().is_empty() {
            out.push_str(&format!(
                "    attributes: {}\n",
                attribute_text(message.attributes())
            ));
        }
        for signal in message.signals() {
            out.push_str(&format!(
                "    SG_ {} start={} bits={} order={} type={} factor={} offset={} min={} max={} unit=\"{}\" receivers={}\n",
                signal.name(),
                signal.start_bit(),
                signal.signal_size(),
                byte_order_label(signal.byte_order()),
                value_type_label(signal.value_type()),
                signal.factor(),
                signal.offset(),
                signal.min(),
                signal.max(),
                signal.unit(),
                if signal.receivers().is_empty() {
                    "Vector__XXX".to_string()
                } else {
                    signal.receivers().join(",")
                }
            ));
            if !signal.comment().is_empty() {
                out.push_str(&format!("        comment: {}\n", signal.comment()));
            }
            if !signal.value_descriptions().is_empty() {
                out.push_str(&format!(
                    "        values: {}\n",
                    signal
                        .value_descriptions()
                        .iter()
                        .map(|(v, d)| format!("{v}=\"{d}\""))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            if !signal.attributes().is_empty() {
                out.push_str(&format!(
                    "        attributes: {}\n",
                    attribute_text(signal.attributes())
                ));
            }
        }
        out.push('\n');
    }
    out
}

fn attribute_text(attributes: &[(String, AttrValue)]) -> String {
    attributes
        .iter()
        .map(|(name, value)| format!("{name}={}", value.display()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn json_database(loaded: &Loaded, only: Option<u32>) -> String {
    let dbc = &loaded.dbc;
    let mut out = String::new();
    out.push_str(&format!(
        "{{\"file\":{},\"encoding\":\"{}\",\"nodes\":[{}],\"messages\":[",
        json_str(&loaded.path.display().to_string()),
        loaded.encoding.name(),
        json_array(dbc.nodes().iter().map(|n| n.as_str()))
    ));
    let messages: Vec<&EditableMessage> = dbc
        .messages()
        .iter()
        .filter(|m| only.is_none_or(|id| m.message_id() == id))
        .collect();
    for (index, message) in messages.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let cycle = dbc.message_cycle_time(message.message_id());
        out.push_str(&format!(
            "{{\"id\":{},\"idHex\":\"{}\",\"name\":{},\"frameFormat\":\"{}\",\"size\":{},\"transmitter\":{},\"comment\":{},\"cycle\":{},\"attributes\":{},\"signals\":[",
            message.message_id(),
            format_id(message).trim_start_matches("0x"),
            json_str(message.message_name()),
            frame_label(message.frame_format()),
            message.message_size(),
            json_str(message.transmitter()),
            json_str(message.comment()),
            match &cycle {
                Some(value) => json_str(&value.display()),
                None => "null".to_string(),
            },
            json_object(message.attributes().iter().map(|(name, value)| {
                (name.as_str(), value.display())
            }))
        ));
        for (signal_index, signal) in message.signals().iter().enumerate() {
            if signal_index > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"name\":{},\"start\":{},\"bits\":{},\"byteOrder\":\"{}\",\"valueType\":\"{}\",\"factor\":{},\"offset\":{},\"min\":{},\"max\":{},\"unit\":{},\"receivers\":[{}],\"comment\":{},\"values\":{},\"attributes\":{}}}",
                json_str(signal.name()),
                signal.start_bit(),
                signal.signal_size(),
                byte_order_label(signal.byte_order()),
                value_type_label(signal.value_type()),
                signal.factor(),
                signal.offset(),
                signal.min(),
                signal.max(),
                json_str(signal.unit()),
                json_array(signal.receivers().iter().map(|n| n.as_str())),
                json_str(signal.comment()),
                json_object(signal.value_descriptions().iter().map(|(v, d)| (v.to_string(), d.clone()))),
                json_object(
                    signal
                        .attributes()
                        .iter()
                        .map(|(name, value)| (name.as_str(), value.display())),
                )
            ));
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
}

fn json_array<'a>(items: impl Iterator<Item = &'a str>) -> String {
    items.map(json_str).collect::<Vec<_>>().join(",")
}

/// 属性与值表都成 `"名字": "取值"`；值一律是字符串，类型由 `BA_DEF_` 决定
fn json_object<'a>(
    items: impl Iterator<Item = (impl std::fmt::Display + 'a, impl std::fmt::Display + 'a)>,
) -> String {
    items
        .map(|(name, value)| {
            format!(
                "{}:{}",
                json_str(&name.to_string()),
                json_str(&value.to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// JSON 字符串字面量：控制字符转义，非 ASCII 原样写出（JSON 允许 UTF-8）
fn json_str(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// release 版是 `windows_subsystem = "windows"`：进程启动时没有控制台，Rust 缓存的
/// 标准输出句柄是空的，`println!` 会静默丢掉。所以命令行输出一律自己取句柄写——
/// 继承来的句柄可用就用（`> file`、管道），否则接上父控制台再开 `CONOUT$`（在终端里
/// 直接跑）。双击启动没有父控制台，写不出去也不报错。
#[cfg(windows)]
pub fn write_stdout(text: &str) {
    console::write(console::STD_OUT, text);
}

#[cfg(windows)]
pub fn write_stderr(text: &str) {
    console::write(console::STD_ERR, text);
}

#[cfg(not(windows))]
pub fn write_stdout(text: &str) {
    use std::io::Write as _;
    let _ = std::io::stdout().write_all(text.as_bytes());
}

#[cfg(not(windows))]
pub fn write_stderr(text: &str) {
    use std::io::Write as _;
    let _ = std::io::stderr().write_all(text.as_bytes());
}

/// 输出是自己接上父控制台写掉的（而不是管道或文件）时为真：shell 不等这种 GUI
/// 程序，跑完提示符不会自己刷新，所以 main 里补一句让人按回车。重定向时不提，
/// 免得混进脚本和 AI 读到的输出里。
pub fn prompt_needs_redraw() -> bool {
    #[cfg(windows)]
    {
        console::used_conout()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
mod console {
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::Once;
    use std::sync::atomic::{AtomicBool, Ordering};

    pub const STD_OUT: u32 = u32::MAX - 10; // (DWORD)-11
    pub const STD_ERR: u32 = u32::MAX - 11; // (DWORD)-12
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const OPEN_EXISTING: u32 = 3;
    const FILE_TYPE_UNKNOWN: u32 = 0;
    const INVALID: *mut c_void = -1isize as *mut c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> *mut c_void;
        fn GetFileType(handle: *mut c_void) -> u32;
        fn WriteFile(
            handle: *mut c_void,
            buffer: *const u8,
            count: u32,
            written: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *const c_void,
            disposition: u32,
            flags: u32,
            template: *const c_void,
        ) -> *mut c_void;
        fn AttachConsole(process: u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    static ATTACHED: Once = Once::new();
    static USED_CONOUT: AtomicBool = AtomicBool::new(false);

    /// 有没有走过 CONOUT$ 这条路（也就是真的在控制台里跑）
    pub fn used_conout() -> bool {
        USED_CONOUT.load(Ordering::Relaxed)
    }

    fn usable(handle: *mut c_void) -> bool {
        !handle.is_null()
            && handle != INVALID
            && unsafe { GetFileType(handle) != FILE_TYPE_UNKNOWN }
    }

    pub fn write(which: u32, text: &str) {
        if text.is_empty() {
            return;
        }
        let inherited = unsafe { GetStdHandle(which) };
        let (handle, owned) = if usable(inherited) {
            (inherited, false)
        } else {
            ATTACHED.call_once(|| unsafe {
                AttachConsole(ATTACH_PARENT_PROCESS);
            });
            let mut conout: Vec<u16> = "CONOUT$".encode_utf16().collect();
            conout.push(0);
            let opened = unsafe {
                CreateFileW(
                    conout.as_ptr(),
                    GENERIC_WRITE,
                    FILE_SHARE_WRITE,
                    ptr::null(),
                    OPEN_EXISTING,
                    0,
                    ptr::null(),
                )
            };
            (opened, true)
        };
        if !usable(handle) {
            return;
        }
        if owned {
            USED_CONOUT.store(true, Ordering::Relaxed);
        }
        let bytes = text.as_bytes();
        let mut offset = 0;
        unsafe {
            while offset < bytes.len() {
                let mut written = 0u32;
                let count = (bytes.len() - offset).min(u32::MAX as usize) as u32;
                let ok = WriteFile(
                    handle,
                    bytes.as_ptr().add(offset),
                    count,
                    &mut written,
                    ptr::null_mut(),
                );
                if ok == 0 || written == 0 {
                    break;
                }
                offset += written as usize;
            }
            if owned {
                CloseHandle(handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "VERSION \"\"

NS_ :

BS_:

BU_: EngineControl ABS

BO_ 100 EngineData: 4 EngineControl
 SG_ EngineSpeed : 0|16@1+ (0.125,0) [0|8000] \"rpm\" ABS
 SG_ Temperature : 16|8@1+ (1,-40) [-40|215] \"degC\" ABS

CM_ BO_ 100 \"old comment\";
CM_ SG_ 100 EngineSpeed \"speed\";
";

    /// 带 CycleTime 声明的文件：--cycle 应该写到它头上，而不是新造 GenMsgCycleTime
    const CYCLE: &str = "VERSION \"\"

NS_ :

BS_:

BU_: ECU1

BO_ 201 WheelData: 8 ECU1
 SG_ Speed : 0|16@1+ (1,0) [0|0] \"\" Vector__XXX

BA_DEF_ BO_  \"CycleTime\" INT 5 10000;
BA_DEF_DEF_  \"CycleTime\" 100;
BA_ \"CycleTime\" BO_ 201 200;
";

    /// 带 FLOAT 声明与节点注释的文件：验证类型收敛与丢弃提示
    const TYPED: &str = "VERSION \"\"

NS_ :

BS_:

BU_: ECU1 ECU2

BO_ 300 DiagData: 2 ECU1
 SG_ Level : 0|8@1+ (0.1,0) [0|25.5] \"\" ECU2

BA_DEF_ SG_  \"GenSigStartValue\" FLOAT 0 1000;
BA_DEF_DEF_  \"GenSigStartValue\" 0.0;
CM_ BU_ ECU1 \"node text\";
";

    fn workspace(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("roxy-dbc-cli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn dbc_file(name: &str, text: &str) -> PathBuf {
        let path = workspace(name).join("in.dbc");
        std::fs::write(&path, text).expect("write fixture");
        path
    }

    fn run(parts: &[&str]) -> Outcome {
        let argv: Vec<String> = std::iter::once("dbc".to_string())
            .chain(parts.iter().map(|p| p.to_string()))
            .collect();
        execute(&argv)
    }

    fn read(path: &Path) -> String {
        let bytes = std::fs::read(path).expect("read back");
        decode_file_bytes(&bytes).text
    }

    #[test]
    fn comment_replaces_the_message_comment_in_place() {
        let path = dbc_file("comment", BASE);
        let outcome = run(&[
            "comment",
            path.to_str().unwrap(),
            "--message",
            "EngineData",
            "--text",
            "发动机数据",
        ]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(outcome.out.contains("comment BO_ 0x064 EngineData"));
        let text = read(&path);
        assert!(text.contains("CM_ BO_ 100 \"发动机数据\";"), "{text}");
        assert!(!text.contains("old comment"), "{text}");
    }

    #[test]
    fn clear_removes_a_signal_comment_and_only_that_one() {
        let path = dbc_file("clear", BASE);
        let outcome = run(&[
            "comment",
            path.to_str().unwrap(),
            "--signal",
            "EngineData.EngineSpeed",
            "--clear",
        ]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let text = read(&path);
        assert!(!text.contains("CM_ SG_"), "{text}");
        assert!(text.contains("CM_ BO_ 100 \"old comment\";"), "{text}");
    }

    #[test]
    fn a_gbk_file_stays_gbk_after_an_edit() {
        let dir = workspace("gbk");
        let path = dir.join("gbk.dbc");
        let source = BASE.replace("old comment", "旧注释");
        let (bytes, _, _) = encoding_rs::GBK.encode(&source);
        std::fs::write(&path, bytes).unwrap();

        let outcome = run(&[
            "comment",
            path.to_str().unwrap(),
            "--message",
            "100",
            "--text",
            "车辆状态",
        ]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);

        let raw = std::fs::read(&path).unwrap();
        assert!(
            std::str::from_utf8(&raw).is_err(),
            "file was rewritten as UTF-8"
        );
        assert!(read(&path).contains("车辆状态"));
        assert!(
            read(&path).contains("CM_ SG_ 100 EngineSpeed \"speed\""),
            "the signal comment survived the edit"
        );
    }

    #[test]
    fn message_and_signal_add_produce_a_file_the_parser_reads_back() {
        let path = dbc_file("add", BASE);
        let outcome = run(&[
            "message",
            "add",
            path.to_str().unwrap(),
            "--name",
            "BrakeData",
            "--id",
            "0x1A",
            "--size",
            "2",
            "--transmitter",
            "ABS",
            "--text",
            "制动数据",
        ]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let outcome = run(&[
            "signal",
            "add",
            path.to_str().unwrap(),
            "--message",
            "BrakeData",
            "--name",
            "BrakePressure",
            "--start",
            "0",
            "--bits",
            "12",
            "--byte-order",
            "motorola",
            "--factor",
            "0.5",
            "--min",
            "0",
            "--max",
            "200",
            "--unit",
            "bar",
            "--receivers",
            "EngineControl",
            "--values",
            "0 \"idle\" 1 \"active\"",
            "--text",
            "制动压力",
        ]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);

        let text = read(&path);
        let parsed = can_dbc::Dbc::try_from(text.as_str()).expect("reparse");
        let message = parsed
            .messages
            .iter()
            .find(|m| m.name == "BrakeData")
            .expect("message");
        assert_eq!(message.id.raw(), 0x1A);
        assert_eq!(message.size, 2);
        assert_eq!(message.transmitter.as_deref(), Some("ABS"));
        let signal = message
            .signals
            .iter()
            .find(|s| s.name == "BrakePressure")
            .expect("signal");
        assert_eq!(signal.start_bit, 0);
        assert_eq!(signal.size, 12);
        assert_eq!(signal.byte_order, can_dbc::ByteOrder::BigEndian);
        assert_eq!(signal.receivers[0], "EngineControl");
        assert!(text.contains("CM_ BO_ 26 \"制动数据\";"), "{text}");
        assert!(
            text.contains("VAL_ 26 BrakePressure 0 \"idle\" 1 \"active\" ;"),
            "{text}"
        );
    }

    #[test]
    fn cycle_goes_to_the_attribute_the_file_declares() {
        let path = dbc_file("cycle", CYCLE);
        let outcome = run(&[
            "message",
            "set",
            path.to_str().unwrap(),
            "--message",
            "WheelData",
            "--cycle",
            "20",
        ]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(outcome.out.contains("cycle CycleTime"), "{}", outcome.out);
        let text = read(&path);
        assert!(text.contains("BA_ \"CycleTime\" BO_ 201 20;"), "{text}");
        assert!(!text.contains("GenMsgCycleTime"), "{text}");
    }

    #[test]
    fn an_attribute_value_follows_its_declared_type() {
        let path = dbc_file("typed", TYPED);
        let outcome = run(&[
            "attribute",
            "set",
            path.to_str().unwrap(),
            "--signal",
            "DiagData.Level",
            "--name",
            "GenSigStartValue",
            "--value",
            "0",
        ]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let text = read(&path);
        // 声明是 FLOAT，写 0 也要落成 0.0，否则别家的解析器读不回
        assert!(
            text.contains("BA_ \"GenSigStartValue\" SG_ 300 Level 0.0;"),
            "{text}"
        );
    }

    #[test]
    fn node_comments_are_announced_before_writing() {
        let path = dbc_file("dropped", TYPED);
        let outcome = run(&[
            "comment",
            path.to_str().unwrap(),
            "--message",
            "DiagData",
            "--text",
            "诊断",
        ]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(
            outcome.err.contains("node comments") && outcome.err.contains("not written back"),
            "{}",
            outcome.err
        );
        assert!(!read(&path).contains("CM_ BU_"));
    }

    #[test]
    fn an_unknown_target_says_so_and_lists_what_is_there() {
        let path = dbc_file("unknown", BASE);
        let outcome = run(&[
            "comment",
            path.to_str().unwrap(),
            "--message",
            "NoSuchMessage",
            "--text",
            "x",
        ]);
        assert_eq!(outcome.code, 2);
        assert!(outcome.err.contains("no message named"), "{}", outcome.err);
        assert!(
            outcome.err.contains("EngineData (0x064)"),
            "{}",
            outcome.err
        );

        let outcome = run(&[
            "signal",
            "set",
            path.to_str().unwrap(),
            "--signal",
            "EngineData.NoSuchSignal",
            "--min",
            "1",
        ]);
        assert!(
            outcome.err.contains("no signal \"NoSuchSignal\"")
                && outcome.err.contains("EngineSpeed"),
            "{}",
            outcome.err
        );
    }

    #[test]
    fn an_unknown_option_lists_the_options_of_that_command() {
        let path = dbc_file("option", BASE);
        let outcome = run(&["comment", path.to_str().unwrap(), "--colour", "red"]);
        assert_eq!(outcome.code, 2);
        assert!(
            outcome.err.contains("--colour is not an option"),
            "{}",
            outcome.err
        );
        assert!(outcome.err.contains("--text <s>"), "{}", outcome.err);
        assert!(!outcome.err.contains("--start <bit>"), "{}", outcome.err);
    }

    #[test]
    fn a_classic_frame_refuses_more_than_eight_bytes() {
        let path = dbc_file("size", BASE);
        let outcome = run(&[
            "message",
            "add",
            path.to_str().unwrap(),
            "--name",
            "Big",
            "--size",
            "16",
        ]);
        assert_eq!(outcome.code, 2);
        assert!(outcome.err.contains("standard-fd"), "{}", outcome.err);
    }

    #[test]
    fn validate_reports_counts_and_exits_one_on_errors() {
        let path = dbc_file("validate", BASE);
        let outcome = run(&["validate", path.to_str().unwrap()]);
        assert_eq!(outcome.code, 0, "{}", outcome.out);
        assert!(
            outcome.out.contains("0 errors") && outcome.out.contains("warning"),
            "{}",
            outcome.out
        );

        // 因子为 0 是错误：退出码必须是 1，报告仍然打到 stdout
        let broken = dbc_file("validate-bad", &BASE.replace("(0.125,0)", "(0,0)"));
        let outcome = run(&["validate", broken.to_str().unwrap(), "--json"]);
        assert_eq!(outcome.code, 1, "{}", outcome.out);
        assert!(outcome.out.contains("\"errors\":1"), "{}", outcome.out);
    }

    #[test]
    fn show_prints_the_database_as_text_and_as_json() {
        let path = dbc_file("show", BASE);
        let outcome = run(&["show", path.to_str().unwrap(), "--message", "EngineData"]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let text = outcome.out;
        assert!(text.contains("BO_ 0x064 EngineData size=4"), "{text}");
        assert!(text.contains("comment: old comment"), "{text}");
        assert!(text.contains("SG_ EngineSpeed start=0 bits=16"), "{text}");
        assert!(text.contains("factor=0.125"), "{text}");
        assert!(text.contains("comment: speed"), "{text}");

        let outcome = run(&["show", path.to_str().unwrap(), "--json"]);
        let json = outcome.out;
        assert!(json.contains("\"idHex\":\"064\""), "{json}");
        assert!(json.contains("\"comment\":\"old comment\""), "{json}");
        assert!(json.contains("\"values\":{\"0\":\"Off\"}") || !json.contains("\"values\":{}"));
        assert!(json.contains("\"receivers\":[\"ABS\"]"), "{json}");
    }

    #[test]
    fn json_escapes_quotes_and_leaves_chinese_alone() {
        assert_eq!(json_str("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(json_str("发动机"), "\"发动机\"");
        assert_eq!(json_str("line\nbreak"), "\"line\\nbreak\"");
    }

    #[test]
    fn value_tables_are_read_the_way_val_stores_them() {
        let values = parse_value_table("10 \"十\" 2 \"二\"").unwrap();
        assert_eq!(values, vec![(2, "二".into()), (10, "十".into())]);
        assert!(parse_value_table("0=Off; 1=On").is_err());
        assert!(parse_value_table("0 \"Off\" 1").is_err());
        assert!(parse_value_table("0 \"Off\" 0 \"Zero\"").is_err());
    }

    #[test]
    fn help_lists_the_commands_and_one_command_its_options() {
        let outcome = run(&["--help"]);
        assert_eq!(outcome.code, 0);
        assert!(outcome.out.contains("roxy-dbc dbc --"), "{}", outcome.out);
        assert!(outcome.out.contains("attribute set"), "{}", outcome.out);

        // 顶层 `roxy-dbc --help`：第一个参数就是 --help，没有命令名
        let outcome = execute(&["--help".to_string()]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(outcome.out.contains("exit codes"), "{}", outcome.out);

        // 只给 `dbc` 不给命令：说清楚要哪条命令，不 panic
        let outcome = execute(&["dbc".to_string()]);
        assert_eq!(outcome.code, 2);
        assert!(outcome.err.contains("which command?"), "{}", outcome.err);

        // Windows 习惯的 /? 与 -? 也当帮助
        for word in ["/?", "-?"] {
            let outcome = execute(&[word.to_string()]);
            assert_eq!(outcome.code, 0, "{word}: {}", outcome.err);
            assert!(outcome.out.contains("exit codes"), "{word}");
        }

        // help 后面跟命令名：只列那条命令
        let outcome = execute(&["dbc".into(), "help".into(), "comment".into()]);
        assert!(outcome.out.contains("dbc comment FILE"), "{}", outcome.out);
        assert!(outcome.out.contains("--text <s>"), "{}", outcome.out);
        assert!(!outcome.out.contains("--byte-order"), "{}", outcome.out);

        let outcome = execute(&["help".into(), "message".into(), "add".into()]);
        assert!(
            outcome.out.contains("dbc message add FILE"),
            "{}",
            outcome.out
        );

        let outcome = run(&["signal", "add", "--help"]);
        assert!(
            outcome.out.contains("dbc signal add FILE"),
            "{}",
            outcome.out
        );
        assert!(
            outcome.out.contains("--byte-order <order>"),
            "{}",
            outcome.out
        );
        assert!(
            !outcome.out.contains("--transmitter <node>"),
            "{}",
            outcome.out
        );
    }

    #[test]
    fn out_writes_a_second_file_and_backup_keeps_the_first() {
        let path = dbc_file("out", BASE);
        let other = path.parent().unwrap().join("copy.dbc");
        let outcome = run(&[
            "comment",
            path.to_str().unwrap(),
            "--message",
            "EngineData",
            "--text",
            "副本",
            "--out",
            other.to_str().unwrap(),
        ]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(read(&other).contains("副本"));
        assert!(read(&path).contains("old comment"), "input untouched");

        let outcome = run(&[
            "comment",
            path.to_str().unwrap(),
            "--message",
            "EngineData",
            "--text",
            "改过",
            "--backup",
        ]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(read(&path).contains("改过"));
        assert!(
            read(&path.parent().unwrap().join("in.dbc.bak")).contains("old comment"),
            "backup holds the previous content"
        );
    }

    #[test]
    fn nodes_can_be_added_renamed_and_deleted() {
        let path = dbc_file("nodes", BASE);
        assert_eq!(
            run(&["node", "add", path.to_str().unwrap(), "--name", "ABS"]).code,
            2,
            "adding a node that exists must fail"
        );
        let outcome = run(&[
            "node",
            "rename",
            path.to_str().unwrap(),
            "--name",
            "ABS",
            "--to",
            "ABS2",
        ]);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let text = read(&path);
        assert!(text.contains("BU_: EngineControl ABS2"), "{text}");
        assert!(
            text.contains("\"rpm\" ABS2"),
            "the receiver reference followed: {text}"
        );
    }
}
