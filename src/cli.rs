use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "todo",
    version,
    about = "一个简单好用的终端 Todo 待办事项管理工具",
    arg_required_else_help = false
)]
pub struct Cli {
    /// 以 JSON 格式输出结果（机器/Agent 友好）
    #[arg(long, global = true)]
    pub json: bool,

    /// 快速添加一条待办事项（快捷语法糖）
    #[arg(
        short = 'n',
        long = "new",
        value_name = "TEXT",
        conflicts_with_all = ["list", "command"]
    )]
    pub new_title: Option<String>,

    /// 列出当前未完成的待办事项（快捷语法糖）
    #[arg(short = 'l', long = "list", conflicts_with_all = ["new_title", "command"])]
    pub list: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// 添加一条或多条待办事项（若参数为 "-" 则从标准输入逐行读取）
    Add {
        /// 待办内容（支持传入多段文字或 "-"）
        #[arg(required = true)]
        titles: Vec<String>,
    },

    /// 标记一个或多个待办事项为已完成
    #[command(alias = "check")]
    Done {
        /// 待办事项编号 ID
        #[arg(required = true)]
        ids: Vec<u64>,
    },

    /// 重新标记一个或多个待办事项为未完成
    Undo {
        /// 待办事项编号 ID
        #[arg(required = true)]
        ids: Vec<u64>,
    },

    /// 删除一个或多个待办事项
    #[command(alias = "delete")]
    Rm {
        /// 待办事项编号 ID
        #[arg(required = true)]
        ids: Vec<u64>,
    },

    /// 修改指定待办事项的内容
    Edit {
        /// 待办事项编号 ID
        id: u64,
        /// 新的待办内容
        title: String,
    },

    /// 列出待办事项
    #[command(alias = "ls")]
    List {
        /// 列出全部待办事项（包括未完成与已完成）
        #[arg(short = 'a', long)]
        all: bool,

        /// 仅列出已完成的待办事项
        #[arg(short = 'd', long, conflicts_with = "all")]
        done: bool,

        /// 限制输出条数
        #[arg(short = 'L', long)]
        limit: Option<usize>,
    },

    /// 搜索待办事项
    #[command(alias = "find")]
    Search {
        /// 搜索关键词
        keyword: String,
    },

    /// 清理待办事项
    Clear {
        /// 仅清理已完成的待办事项（默认行为）
        #[arg(short = 'd', long, default_value_t = true)]
        done: bool,
    },

    /// 查看待办统计数据指标
    Stats,
}
