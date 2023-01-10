#![allow(unused_imports,unused_variables)]
use std::collections::HashMap;
use sqlparser::dialect::MySqlDialect;
use sqlparser::tokenizer::{Token, Tokenizer};

pub fn is_pattern_match(pattern: &str, sql2: &str, dialect: &MySqlDialect) -> bool {
    let tokens1: Vec<Token> = Tokenizer::new(dialect, pattern)
        .tokenize()
        .unwrap_or_default();
    let tokens2: Vec<Token> = Tokenizer::new(dialect, sql2).tokenize().unwrap_or_default();

    let tokens1: Vec<Token> = tokens1
        .into_iter()
        .filter(|t| {
            return match t {
                Token::EOF => false,
                Token::Whitespace(_) => false,
                _ => true,
            };
        })
        .collect();
    let tokens2: Vec<Token> = tokens2
        .into_iter()
        .filter(|t| {
            return match t {
                Token::EOF => false,
                Token::Whitespace(_) => false,
                _ => true,
            };
        })
        .collect();
    trace!("tokens1:{:?}\ntokens2:{:?}\ntokens1.len:{:?},tokens2.len:{:?}", tokens1, tokens2, tokens1.len(), tokens2.len());
    if tokens1.len() != tokens2.len() {
        return false;
    }

    for index in 0..tokens1.len() {
        let a = &tokens1[index];
        let b = &tokens2[index];
        trace!("sql match token pair. a:{:?},b:{:?}", a, b);
        let is_same = match (a, b) {
            (Token::Word(v_a), Token::Word(v_b)) => {
                v_a.value == v_b.value
            }
            (Token::Number(v_a_str, v_a_bool), Token::Number(v_b_str, v_b_bool)) => {
                v_a_str == v_b_str && v_a_bool == v_b_bool
            }
            (Token::Char(v_a), Token::Char(v_b)) => {
                v_a == v_b
            }
            (Token::SingleQuotedString(v_a), Token::SingleQuotedString(v_b)) => {
                v_a == v_b
            }
            (Token::NationalStringLiteral(v_a), Token::NationalStringLiteral(v_b)) => {
                v_a == v_b
            }
            (Token::EscapedStringLiteral(v_a), Token::EscapedStringLiteral(v_b)) => {
                true
            }
            (Token::HexStringLiteral(v_a), Token::HexStringLiteral(v_b)) => {
                v_a == v_b
            }
            (Token::Whitespace(v_a), Token::Whitespace(v_b)) => {
                true
            }
            (Token::Placeholder(v_a), Token::Placeholder(v_b)) => {
                true
            }
            _ => {
                a == b
            }
        };
        if !is_same {
            trace!("a != b,return");
            return false;
        }
    }
    return true;
}


#[test]
fn test_match() {
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "trace");
    env_logger::Builder::from_env(env)
        .build();
    let sql = "SELECT * FROM article limit 1000,10;";
    let pattern = "SELECT * FROM `article` limit 1000,10;";
    let dialect = MySqlDialect {}; // or AnsiDialect, or your own dialect ...
    let matched = is_pattern_match(sql, pattern, &dialect);
    trace!("pattern:{:?}\nsql:{:?}\neq:{:?}", pattern, sql, matched);
}

#[test]
fn test_matchs() {
    let dialect = MySqlDialect {}; // or AnsiDialect, or your own dialect ...

    let mut sqls: HashMap<&str, &str> = HashMap::new();
    sqls.insert(
        "select count(1) from article where channel_id = ?  and tenant_id = ?",
        "select count(1) from article where channel_id = 312  and tenant_id = 1",
    );
    sqls.insert("select count(1) from article where belong_user_id = ? and article_oper_type <> ? and article_status = ? and app_id = ? and tenant_id = ?", "select count(1) from article where belong_user_id = 1 and article_oper_type <> 2 and article_status = 11 and app_id = 0 and tenant_id = 1");
    sqls.insert(
        "select count(1) from article",
        "select count(1) from article",
    );
    sqls.insert(
        "select * from article order by article_id desc limit ?",
        "select * from article order by article_id desc limit 100",
    );
    sqls.insert("select channel_id,count(channel_id) from article group by channel_id having count(channel_id) > ? order by count(channel_id) desc", "select channel_id,count(channel_id) from article group by channel_id having count(channel_id) > 123 order by count(channel_id) desc");
    sqls.insert("select count(1) from article where belong_dept_id = ? and article_oper_type <> ? and article_status =? and app_id = ? and tenant_id = ?", "select count(1) from article where belong_dept_id = 123 and article_oper_type <> 2 and article_status =43 and app_id = 0 and tenant_id = 1");
    sqls.insert("select  str_to_date(publish_time,'%Y-%m-%d') as date,count(publish_time) as count from article where channel_id = ? and article_status = ? and tenant_id = ? and str_to_date(publish_time,'%Y-%m-%d') >= str_to_date(?,'%Y-%m-%d') and str_to_date(publish_time,'%Y-%m-%d')  <= str_to_date(?,'%Y-%m-%d') GROUP BY str_to_date(publish_time,'%Y-%m-%d') ORDER BY publish_time asc", "select  str_to_date(publish_time,'%Y-%m-%d') as date,count(publish_time) as count from article where channel_id = 21 and article_status =3 and tenant_id = 1 and str_to_date(publish_time,'%Y-%m-%d') >= str_to_date('2000-01-01 00:00:00','%Y-%m-%d') and str_to_date(publish_time,'%Y-%m-%d')  <= str_to_date('2000-01-01','%Y-%m-%d') GROUP BY str_to_date(publish_time,'%Y-%m-%d') ORDER BY publish_time asc");
    sqls.insert("SELECT a.article_title,a.article_id,a.article_type,c.channel_name,a.article_status,a.update_time from article a LEFT JOIN channel c on a.channel_id = c.channel_id where a.tenant_id = ? and a.article_id IN(?,?,?) order by field( a.article_id,?,?,?)", "SELECT a.article_title,a.article_id,a.article_type,c.channel_name,a.article_status,a.update_time from article a LEFT JOIN channel c on a.channel_id = c.channel_id where a.tenant_id = 1 and a.article_id IN(1,2,3) order by field( a.article_id,4,5,6)");
    sqls.insert("SELECT a.article_id from article a where a.tenant_id = ? and a.app_id = ? and a.article_status = ? order by a.publish_time desc limit ?,?", "SELECT a.article_id from article a where a.tenant_id = 1 and a.app_id = 4 and a.article_status = 12 order by a.publish_time desc limit 0,123");
    sqls.insert("SELECT a.article_id,a.article_title,a.article_author,a.publish_time,a.click_num from article a  where a.tenant_id = ? and a.article_id IN (?,?,?) order by field( a.article_id,?,?,?)", "SELECT a.article_id,a.article_title,a.article_author,a.publish_time,a.click_num from article a  where a.tenant_id = 1 and a.article_id IN (1,2,3) order by field( a.article_id,1,2,3)");
    sqls.insert("SELECT a.*, b.content FROM article a LEFT JOIN article_content b ON a.article_content_id = b.article_content_id WHERE a.tenant_id = ? AND app_id = ? AND a.article_id IN ( ?, ?, ? )", "SELECT a.*, b.content FROM article a LEFT JOIN article_content b ON a.article_content_id = b.article_content_id WHERE a.tenant_id = 1 AND app_id = 0 AND a.article_id IN ( 1,2,3 )");
    sqls.insert("select /*+ QUERY_TIMEOUT(100000000) */ count(1) from article where channel_id = ? and article_oper_type <> ? and article_status = ? and tenant_id = ? and publish_time LIKE CONCAT(?,'%')", "select /*+ QUERY_TIMEOUT(100000000) */ count(1) from article where channel_id = 2 and article_oper_type <> 2 and article_status =1 and tenant_id = 1 and publish_time LIKE CONCAT(11,'%')");

    for (pattern, sql) in sqls {
        let matched = is_pattern_match(pattern, sql, &dialect);
        println!("pattern:{:?}\nsql:{:?}\neq:{:?}\n", pattern, sql, matched);
        assert_eq!(true, matched);
    }
}
