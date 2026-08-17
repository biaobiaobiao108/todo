use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local};
use rusqlite::{Connection, params};

use crate::model::Todo;

pub struct TodoStore {
    conn: Connection,
    data: Vec<Todo>,
}

impl TodoStore {
    pub fn load() -> Result<Self> {
        let path = data_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("无法创建 {}", parent.display()))?;
        }
        Self::open(path)
    }

    fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("无法打开数据库 {}", path.as_ref().display()))?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS todos (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 title TEXT NOT NULL,
                 completed INTEGER NOT NULL DEFAULT 0,
                 created_at TEXT NOT NULL,
                 completed_at TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_todos_completed_created
                 ON todos (completed, created_at);",
        )
        .context("无法初始化 Todo 数据表")?;
        let data = load_items(&conn)?;
        Ok(Self { conn, data })
    }

    pub fn items(&self) -> &[Todo] {
        &self.data
    }

    pub fn add(&mut self, title: String) -> Result<&Todo> {
        let title = title.trim().to_owned();
        if title.is_empty() {
            bail!("待办内容不能为空");
        }

        let created_at = Local::now();
        self.conn
            .execute(
                "INSERT INTO todos (title, completed, created_at) VALUES (?1, 0, ?2)",
                params![title, created_at.to_rfc3339()],
            )
            .context("无法新增待办")?;
        let id = self.conn.last_insert_rowid() as u64;
        self.data.push(Todo {
            id,
            title,
            completed: false,
            created_at,
            completed_at: None,
        });
        Ok(self.data.last().expect("刚刚插入的待办应当存在"))
    }

    pub fn toggle(&mut self, index: usize) -> Result<()> {
        let item = self.data.get_mut(index).context("找不到选中的待办")?;
        let completed = !item.completed;
        let completed_at = completed.then(Local::now);
        self.conn
            .execute(
                "UPDATE todos SET completed = ?1, completed_at = ?2 WHERE id = ?3",
                params![
                    completed as i64,
                    completed_at.as_ref().map(DateTime::to_rfc3339),
                    item.id as i64
                ],
            )
            .context("无法更新待办状态")?;
        item.completed = completed;
        item.completed_at = completed_at;
        Ok(())
    }

    pub fn update_title(&mut self, index: usize, title: String) -> Result<()> {
        let title = title.trim().to_owned();
        if title.is_empty() {
            bail!("待办内容不能为空");
        }
        let item = self.data.get_mut(index).context("找不到选中的待办")?;
        self.conn
            .execute(
                "UPDATE todos SET title = ?1 WHERE id = ?2",
                params![title, item.id as i64],
            )
            .context("无法修改待办内容")?;
        item.title = title;
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Result<()> {
        let item = self.data.get(index).context("找不到选中的待办")?;
        self.conn
            .execute("DELETE FROM todos WHERE id = ?1", params![item.id as i64])
            .context("无法删除待办")?;
        self.data.remove(index);
        Ok(())
    }
}

fn load_items(conn: &Connection) -> Result<Vec<Todo>> {
    let mut statement = conn
        .prepare(
            "SELECT id, title, completed, created_at, completed_at
             FROM todos ORDER BY id ASC",
        )
        .context("无法查询待办列表")?;
    let rows = statement
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let title = row.get(1)?;
            let completed: i64 = row.get(2)?;
            let created_at: String = row.get(3)?;
            let completed_at: Option<String> = row.get(4)?;
            Ok((id, title, completed, created_at, completed_at))
        })
        .context("无法读取待办列表")?;

    rows.map(|row| {
        let (id, title, completed, created_at, completed_at) = row.context("无法读取待办记录")?;
        let created_at = parse_time(&created_at)?;
        let completed_at = completed_at.as_deref().map(parse_time).transpose()?;
        Ok(Todo {
            id: u64::try_from(id).context("待办编号超出支持范围")?,
            title,
            completed: completed != 0,
            created_at,
            completed_at,
        })
    })
    .collect()
}

fn parse_time(value: &str) -> Result<DateTime<Local>> {
    DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("无法解析时间：{value}"))
        .map(|time| time.with_timezone(&Local))
}

fn data_path() -> Result<PathBuf> {
    let base = dirs::data_dir().context("无法确定用户数据目录")?;
    Ok(base.join("todo").join("todos.db"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::OptionalExtension;

    fn test_store() -> TodoStore {
        let path = std::env::temp_dir().join(format!("todo-test-{}.db", std::process::id()));
        let _ = fs::remove_file(&path);
        TodoStore::open(path).expect("测试数据库应当可以打开")
    }

    #[test]
    fn empty_title_is_rejected() {
        let mut store = test_store();
        assert!(store.add("  ".to_owned()).is_err());
    }

    #[test]
    fn persists_add_toggle_and_remove() {
        let path =
            std::env::temp_dir().join(format!("todo-persist-test-{}.db", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut store = TodoStore::open(&path).expect("测试数据库应当可以打开");
        let item = store.add("测试 SQLite".to_owned()).expect("新增应成功");
        assert_eq!(item.id, 1);
        assert_eq!(store.items().len(), 1);

        // 修改测试
        store.update_title(0, "修改后的标题".to_owned()).expect("修改应成功");
        assert_eq!(store.items()[0].title, "修改后的标题");

        store.toggle(0).expect("完成应成功");
        assert!(store.items()[0].completed);
        drop(store);

        let mut reopened = TodoStore::open(&path).expect("数据库应当可以重新打开");
        assert_eq!(reopened.items()[0].title, "修改后的标题");
        assert!(reopened.items()[0].completed);
        reopened.remove(0).expect("删除应成功");
        assert!(reopened.items().is_empty());
        drop(reopened);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn initializes_existing_database_without_json() {
        let path = std::env::temp_dir().join(format!("todo-schema-test-{}.db", std::process::id()));
        let _ = fs::remove_file(&path);
        let store = TodoStore::open(&path).expect("数据库应当可以初始化");
        let table: Option<String> = store
            .conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'todos'",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("应当可以查询表");
        assert_eq!(table.as_deref(), Some("todos"));
        drop(store);
        let _ = fs::remove_file(path);
    }
}
