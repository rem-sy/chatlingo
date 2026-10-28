//! 术语库 CRUD + 匹配逻辑直测（临时库，不碰本机数据目录）
use chatlingo_lib::store::{
    glossary_clear_conn, glossary_delete_conn, glossary_enabled_conn, glossary_export_conn,
    glossary_import_conn, glossary_init, glossary_list_conn, glossary_match_in,
    glossary_norm, glossary_upsert_conn, GlossaryEntry,
};
use rusqlite::Connection;

fn entry(id: i64, source: &str, target: &str, enabled: bool) -> GlossaryEntry {
    GlossaryEntry { id, source: source.into(), target: target.into(), note: String::new(), enabled, created_at: 0 }
}

fn main() {
    let conn = Connection::open_in_memory().unwrap();
    glossary_init(&conn).unwrap();

    // CRUD
    let id1 = glossary_upsert_conn(&conn, &entry(0, "RAG", "检索增强生成", true)).unwrap();
    let id2 = glossary_upsert_conn(&conn, &entry(0, "LLM", "大语言模型", true)).unwrap();
    assert!(id1 > 0 && id2 > 0 && id1 != id2, "ids: {id1} {id2}");
    // 同 source 再插入 → 更新而非新增
    let id1b = glossary_upsert_conn(&conn, &entry(0, "RAG", "检索增强生成（RAG）", true)).unwrap();
    assert_eq!(id1, id1b, "upsert by source should update same row");
    assert_eq!(glossary_list_conn(&conn, "").unwrap().len(), 2);
    // 停用
    glossary_upsert_conn(&conn, &entry(id2, "LLM", "大语言模型", false)).unwrap();
    assert_eq!(glossary_enabled_conn(&conn).unwrap().len(), 1, "disabled entry excluded");
    // 删除
    assert_eq!(glossary_delete_conn(&conn, id1).unwrap(), 1);
    assert_eq!(glossary_list_conn(&conn, "").unwrap().len(), 1);
    println!("[crud] OK");

    // 重建：RAG + 大小写/全角归一 + 冲突（同 source 取 id 最小）+ 截断
    glossary_clear_conn(&conn).unwrap();
    let mut ids = vec![];
    for (s, t) in [("RAG", "检索增强生成"), ("rag pipeline", "RAG 流水线"), ("LLM", "大语言模型")] {
        ids.push(glossary_upsert_conn(&conn, &entry(0, s, t, true)).unwrap());
    }
    // 冲突：同 source 不同 target（绕过 upsert 直接 SQL 插入，模拟异常数据/旧库）
    conn.execute(
        "INSERT INTO glossary (source, target, note, enabled, created_at) VALUES ('RAG', '错误译法', '', 1, 0)",
        [],
    )
    .unwrap();

    let enabled = glossary_enabled_conn(&conn).unwrap();
    // 同 source 两条 enabled → 只剩 id 最小的 RAG 译法
    let rag = enabled.iter().find(|e| e.source == "RAG").unwrap();
    assert_eq!(rag.target, "检索增强生成", "conflict resolves to smallest id");
    println!("[conflict] OK (RAG -> {})", rag.target);

    let text = "帮我实现一个 ＲＡＧ 系统，含 rag pipeline 与 LLM";
    let hits = glossary_match_in(&enabled, text, 30);
    let sources: Vec<&str> = hits.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(sources, vec!["rag pipeline", "LLM", "RAG"], "matched: {sources:?} (longest first, full-width/case-insensitive)");
    println!("[match] OK {sources:?}");

    // 截断
    let hits2 = glossary_match_in(&enabled, text, 1);
    assert_eq!(hits2.len(), 1);
    println!("[truncate] OK");

    // 归一化
    assert_eq!(glossary_norm("ＲＡＧ  Ｔｅｓｔ"), "rag  test");
    println!("[norm] OK");

    // 导入导出往返
    let json = glossary_export_conn(&conn).unwrap();
    println!("[export] {json}");
    glossary_clear_conn(&conn).unwrap();
    let (imported, skipped) = glossary_import_conn(&conn, &json).unwrap();
    // 注意：导出里有两条同 source 的 RAG（冲突模拟数据），导入按 source 去重：
    // 4 条都被处理（imported=4），落库为 3 行
    assert_eq!(imported, 4, "imported={imported} skipped={skipped}");
    assert_eq!(glossary_list_conn(&conn, "").unwrap().len(), 3);
    // CSV 导入（含表头 + 停用列）
    let csv = "source,target,note,enabled\nTransformer,变换器,,1\nMoE,混合专家,,0\n";
    let (imported, _) = glossary_import_conn(&conn, csv).unwrap();
    assert_eq!(imported, 2);
    assert_eq!(glossary_list_conn(&conn, "").unwrap().len(), 5);
    println!("[import/export roundtrip] OK");

    println!("ALL GLOSSARY TESTS PASSED");
}
