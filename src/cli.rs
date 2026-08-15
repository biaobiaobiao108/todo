use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "todo", version, about = "一个简单好用的终端 Todo 工具")]
pub struct Cli {
    /// 快速添加一条待办事项
    #[arg(
        short = 'n',
        long = "new",
        value_name = "TEXT",
        conflicts_with = "list"
    )]
    pub new_title: Option<String>,

    /// 列出当前未完成的待办事项
    #[arg(short = 'l', long = "list", conflicts_with = "new_title")]
    pub list: bool,
}
