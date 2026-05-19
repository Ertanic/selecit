use crate::proto::client::LOGS_MANAGER;
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::{
        complete::{take_till, take_while},
        tag,
    },
    character::complete::{char, space0, space1},
    combinator::{map, map_opt, opt},
    multi::separated_list0,
    number::complete::double,
    sequence::{delimited, separated_pair},
};
use std::fmt::Display;

#[derive(Debug, PartialEq)]
pub struct QueryParseError;

#[derive(Debug, PartialEq, Clone)]
pub struct InvokeFuncArg {
    pub name: String,
    pub value: String,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InvokeFunc {
    pub name: String,
    pub args: Vec<InvokeFuncArg>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum QueryValue {
    FnField { func: InvokeFunc, field: String },
    Identifier(String),
    String(String),
    Number(f64),
    Bool(bool),
    Null,
}

impl Display for QueryValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            QueryValue::FnField { func, field } => format!(
                "{}({}).{field}",
                func.name,
                func.args
                    .iter()
                    .map(|arg| format!("{}={}", arg.name, arg.value))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            QueryValue::Identifier(s) => s.clone(),
            QueryValue::String(s) => s.clone(),
            QueryValue::Number(n) => n.to_string(),
            QueryValue::Bool(b) => b.to_string(),
            QueryValue::Null => "null".to_string(),
        };
        write!(f, "{}", str)
    }
}

#[derive(Debug, PartialEq)]
enum QueryOperation {
    Eq,
    More,
    Less,
}

#[derive(Debug, PartialEq)]
pub enum QueryConditionTerm {
    Eq {
        left: Box<QueryConditionTerm>,
        right: Box<QueryConditionTerm>,
    },
    More {
        left: Box<QueryConditionTerm>,
        right: Box<QueryConditionTerm>,
    },
    Less {
        left: Box<QueryConditionTerm>,
        right: Box<QueryConditionTerm>,
    },
    Value(QueryValue),
}

#[derive(Debug, PartialEq)]
enum QueryExprOperation {
    And,
    Or,
}

#[derive(Debug, PartialEq)]
pub enum QueryConditionExpr {
    And {
        left: Box<QueryConditionExpr>,
        right: Box<QueryConditionExpr>,
    },
    Or {
        left: Box<QueryConditionExpr>,
        right: Box<QueryConditionExpr>,
    },
    Term(QueryConditionTerm),
}

#[derive(Debug, PartialEq)]
pub enum QueryExpr {
    ListBy { field: String, condition: Option<QueryConditionExpr> },
    SelectFrom { from: String, select: Vec<InvokeFunc> },
}

fn identifier(input: &str) -> IResult<&str, QueryValue> {
    map_opt(take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-'), |s: &str| {
        if s.is_empty() {
            None
        }
        else {
            Some(QueryValue::Identifier(s.to_string()))
        }
    })
    .parse(input)
}

fn string(input: &str) -> IResult<&str, QueryValue> {
    map_opt(delimited(char('"'), take_till(|c: char| c == '"'), char('"')), |s: &str| {
        if s.is_empty() { None } else { Some(QueryValue::String(s.to_string())) }
    })
    .parse(input)
}

fn number(input: &str) -> IResult<&str, QueryValue> {
    map(double, QueryValue::Number).parse(input)
}

fn boolean(input: &str) -> IResult<&str, QueryValue> {
    map(alt((tag("true"), tag("false"))), |s: &str| QueryValue::Bool(s == "true")).parse(input)
}

fn null(input: &str) -> IResult<&str, QueryValue> {
    map(tag("null"), |_| QueryValue::Null).parse(input)
}

fn func_field(input: &str) -> IResult<&str, QueryValue> {
    map(separated_pair(invoke_func, char('.'), identifier), |(f, i)| QueryValue::FnField {
        func: f,
        field: i.to_string(),
    })
    .parse(input)
}

fn value(input: &str) -> IResult<&str, QueryValue> {
    alt((string, func_field, identifier, number, boolean, null)).parse(input)
}

fn operation(input: &str) -> IResult<&str, QueryOperation> {
    alt((
        map(char('='), |_| QueryOperation::Eq),
        map(char('<'), |_| QueryOperation::Less),
        map(char('>'), |_| QueryOperation::More),
    ))
    .parse(input)
}

fn condition_term(input: &str) -> IResult<&str, QueryConditionTerm> {
    let (input, left) = map(value, QueryConditionTerm::Value).parse(input)?;
    let (input, _) = space0(input)?;

    let (input, op) = opt(operation).parse(input)?;
    if let Some(op) = op {
        let (input, _) = space0(input)?;
        let (input, right) = condition_term(input)?;

        Ok((
            input,
            match op {
                QueryOperation::Eq => QueryConditionTerm::Eq {
                    left: Box::new(left),
                    right: Box::new(right),
                },
                QueryOperation::More => QueryConditionTerm::More {
                    left: Box::new(left),
                    right: Box::new(right),
                },
                QueryOperation::Less => QueryConditionTerm::Less {
                    left: Box::new(left),
                    right: Box::new(right),
                },
            },
        ))
    }
    else {
        Ok((input, left))
    }
}

fn condition_expr_op(input: &str) -> IResult<&str, QueryExprOperation> {
    alt((map(char('&'), |_| QueryExprOperation::And), map(char('|'), |_| QueryExprOperation::Or))).parse(input)
}

fn condition_expr(input: &str) -> IResult<&str, QueryConditionExpr> {
    let (input, left) = map(condition_term, QueryConditionExpr::Term).parse(input)?;
    let (input, _) = space0(input)?;

    let (input, op) = opt(condition_expr_op).parse(input)?;
    if let Some(op) = op {
        let (input, _) = space0(input)?;
        let (input, right) = condition_expr(input)?;

        Ok((
            input,
            match op {
                QueryExprOperation::And => QueryConditionExpr::And {
                    left: Box::new(left),
                    right: Box::new(right),
                },
                QueryExprOperation::Or => QueryConditionExpr::Or {
                    left: Box::new(left),
                    right: Box::new(right),
                },
            },
        ))
    }
    else {
        Ok((input, left))
    }
}

fn condition(input: &str) -> IResult<&str, QueryConditionExpr> {
    let (input, _) = space1(input)?;
    let (input, _) = tag("where").parse(input)?;
    let (input, _) = space1(input)?;

    condition_expr(input)
}

fn list_by(input: &str) -> IResult<&str, QueryExpr> {
    let (input, _) = tag("list by").parse(input)?;
    let (input, _) = space1(input)?;
    let (input, field_name) = identifier(input)?;

    let (input, condition) = opt(condition).parse(input)?;

    Ok((
        input,
        QueryExpr::ListBy {
            field: field_name.to_string(),
            condition,
        },
    ))
}

fn invoke_arg(input: &str) -> IResult<&str, InvokeFuncArg> {
    let (input, name) = identifier(input)?;

    let (input, _) = space0(input)?;
    let (input, _) = char('=').parse(input)?;
    let (input, _) = space0(input)?;

    let (input, value) = identifier(input)?;

    Ok((
        input,
        InvokeFuncArg {
            name: name.to_string(),
            value: value.to_string(),
        },
    ))
}

fn invoke_args(input: &str) -> IResult<&str, Vec<InvokeFuncArg>> {
    separated_list0(invoke_list_separate, invoke_arg).parse(input)
}

fn invoke_func(input: &str) -> IResult<&str, InvokeFunc> {
    let (input, name) = map(identifier, |s| s.to_string()).parse(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = char('(').parse(input)?;
    let (input, _) = space0(input)?;
    let (input, args) = invoke_args(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = char(')').parse(input)?;

    Ok((input, InvokeFunc { name, args }))
}

fn invoke_list_separate(input: &str) -> IResult<&str, ()> {
    let (input, _) = space0(input)?;
    let (input, _) = char(',').parse(input)?;
    let (input, _) = space0(input)?;
    Ok((input, ()))
}

fn invoke_list(input: &str) -> IResult<&str, Vec<InvokeFunc>> {
    let (input, first) = invoke_func(input)?;
    let mut acc = vec![first];

    let mut input = input;
    while let Ok((i, _)) = invoke_list_separate(input) {
        let (i, func) = invoke_func(i)?;
        acc.push(func);
        input = i;
    }

    Ok((input, acc))
}

fn select_from(input: &str) -> IResult<&str, QueryExpr> {
    let (input, _) = tag("from").parse(input)?;
    let (input, _) = space1(input)?;
    let (input, from) = map(identifier, |s| s.to_string()).parse(input)?;
    let (input, _) = space1(input)?;

    let (input, _) = tag("select").parse(input)?;
    let (input, _) = space1(input)?;
    let (input, invoke_list) = invoke_list(input)?;

    Ok((input, QueryExpr::SelectFrom { from, select: invoke_list }))
}

pub async fn parse_query(query: &str) -> Result<QueryExpr, QueryParseError> {
    let query = alt((list_by, select_from)).parse(query);

    match query {
        Ok(result) => Ok(result.1),
        Err(err) => {
            LOGS_MANAGER.send_log(format!("parse error: {err}")).await;
            Err(QueryParseError)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_field_parser() {
        let query = "list by name";
        let result = identifier(&query[8..]);
        assert_eq!(result, Ok(("", QueryValue::Identifier("name".to_owned()))));
    }

    #[tokio::test]
    async fn test_parse_list_by() {
        let query = "list by name";
        let result = list_by(query);
        assert_eq!(
            result,
            Ok((
                "",
                QueryExpr::ListBy {
                    field: "name".to_owned(),
                    condition: None
                }
            ))
        );
    }

    #[tokio::test]
    async fn test_parse_list_by_where() {
        let query = "list by name where addr = \"127.0.0.1:8080\"";
        let result = list_by(query);
        assert_eq!(
            result,
            Ok((
                "",
                QueryExpr::ListBy {
                    field: "name".to_owned(),
                    condition: Some(QueryConditionExpr::Term(QueryConditionTerm::Eq {
                        left: Box::new(QueryConditionTerm::Value(QueryValue::Identifier("addr".to_string()))),
                        right: Box::new(QueryConditionTerm::Value(QueryValue::String("127.0.0.1:8080".to_string()))),
                    }))
                }
            ))
        );
    }

    #[tokio::test]
    async fn test_parse_list_by_where_invoke_func_field() {
        let query = "list by name where version().version = \"1.0.0\"";
        let result = list_by(query);
        assert_eq!(
            result,
            Ok((
                "",
                QueryExpr::ListBy {
                    field: "name".to_owned(),
                    condition: Some(QueryConditionExpr::Term(QueryConditionTerm::Eq {
                        left: Box::new(QueryConditionTerm::Value(QueryValue::FnField {
                            func: InvokeFunc {
                                name: "version".to_owned(),
                                args: vec![]
                            },
                            field: "version".to_owned()
                        })),
                        right: Box::new(QueryConditionTerm::Value(QueryValue::String("1.0.0".to_string()))),
                    }))
                }
            ))
        );
    }

    #[tokio::test]
    async fn test_parse_list_by_where_invoke_func_field_with_args() {
        let query = "list by name where info(type = modules).version = \"1.0.0\"";
        let result = list_by(query);
        assert_eq!(
            result,
            Ok((
                "",
                QueryExpr::ListBy {
                    field: "name".to_owned(),
                    condition: Some(QueryConditionExpr::Term(QueryConditionTerm::Eq {
                        left: Box::new(QueryConditionTerm::Value(QueryValue::FnField {
                            func: InvokeFunc {
                                name: "info".to_owned(),
                                args: vec![InvokeFuncArg {
                                    name: "type".to_owned(),
                                    value: "modules".to_owned(),
                                }],
                            },
                            field: "version".to_owned()
                        })),
                        right: Box::new(QueryConditionTerm::Value(QueryValue::String("1.0.0".to_string()))),
                    }))
                }
            ))
        );
    }

    #[tokio::test]
    async fn test_parse_list_by_where_invoke_func_field_with_args_and_other() {
        let query = "list by name where info(type = modules).version = \"version\" & version().version = \"0.1.0\"";
        let result = list_by(query);
        assert_eq!(
            result,
            Ok((
                "",
                QueryExpr::ListBy {
                    field: "name".to_owned(),
                    condition: Some(QueryConditionExpr::And {
                        left: Box::new(QueryConditionExpr::Term(QueryConditionTerm::Eq {
                            left: Box::new(QueryConditionTerm::Value(QueryValue::FnField {
                                func: InvokeFunc {
                                    name: "info".to_string(),
                                    args: vec![InvokeFuncArg {
                                        name: "type".to_string(),
                                        value: "modules".to_string(),
                                    }],
                                },
                                field: "version".to_string()
                            })),
                            right: Box::new(QueryConditionTerm::Value(QueryValue::String("version".to_string()))),
                        })),
                        right: Box::new(QueryConditionExpr::Term(QueryConditionTerm::Eq {
                            left: Box::new(QueryConditionTerm::Value(QueryValue::FnField {
                                func: InvokeFunc {
                                    name: "version".to_string(),
                                    args: vec![],
                                },
                                field: "version".to_string()
                            })),
                            right: Box::new(QueryConditionTerm::Value(QueryValue::String("0.1.0".to_string()))),
                        })),
                    })
                }
            ))
        );
    }

    #[tokio::test]
    async fn test_parse_identifier_parser() {
        let query = "from name select version()";
        let (rest, _) = tag::<_, _, nom::error::Error<&str>>("from").parse(query).unwrap();
        let (rest, _) = space1::<_, nom::error::Error<&str>>(rest).unwrap();

        let result = identifier(rest);

        assert_eq!(result, Ok((" select version()", QueryValue::Identifier("name".to_string()))));
    }

    #[tokio::test]
    async fn test_parse_space_parser() {
        let query = "from name select version()";
        let (rest, _) = tag::<_, _, nom::error::Error<&str>>("from").parse(query).unwrap();
        let result = space1::<_, nom::error::Error<&str>>(rest);
        assert_eq!(result, Ok(("name select version()", " ")));
    }

    #[tokio::test]
    async fn test_parse_more_space_parser() {
        let query = "from     name select version()";
        let (rest, _) = tag::<_, _, nom::error::Error<&str>>("from").parse(query).unwrap();
        let result = space1::<_, nom::error::Error<&str>>(rest);
        assert_eq!(result, Ok(("name select version()", "     ")));
    }

    #[tokio::test]
    async fn test_parse_from_parser() {
        let query = "from name select version()";
        let result = tag::<_, _, nom::error::Error<&str>>("from").parse(query);
        assert_eq!(result, Ok((" name select version()", "from")));
    }

    #[tokio::test]
    async fn test_parse_maybe_space_no_spaces() {
        let query = "";
        let result = space0::<_, nom::error::Error<&str>>(query);
        assert_eq!(result, Ok(("", "")))
    }

    #[tokio::test]
    async fn test_parse_maybe_space() {
        let query = "   ,";
        let result = space0::<_, nom::error::Error<&str>>(query);
        assert_eq!(result, Ok((",", "   ")))
    }

    #[tokio::test]
    async fn test_parse_invoke_list_separate() {
        let query = " ,  hello";
        let result = invoke_list_separate(query);
        assert_eq!(result, Ok(("hello", ())));
    }

    #[tokio::test]
    async fn test_parse_invoke_list_separate_without_space() {
        let query = ",hello";
        let result = invoke_list_separate(query);
        assert_eq!(result, Ok(("hello", ())));
    }

    #[tokio::test]
    async fn test_parse_no_invoke_args() {
        let query = "()";
        let (input, _) = char::<_, nom::error::Error<&str>>('(').parse(query).unwrap();
        let result = invoke_args(input);
        assert_eq!(result, Ok((")", vec![])));
    }

    #[tokio::test]
    async fn test_parse_invoke_args() {
        let query = "one = first, two = second)";
        let result = invoke_args(query);
        assert_eq!(
            result,
            Ok((
                ")",
                vec![
                    InvokeFuncArg {
                        name: "one".to_owned(),
                        value: "first".to_owned()
                    },
                    InvokeFuncArg {
                        name: "two".to_owned(),
                        value: "second".to_owned()
                    }
                ]
            ))
        )
    }

    #[tokio::test]
    async fn test_parse_invoke_func() {
        let query = "version()";
        let result = invoke_func(query);
        assert_eq!(
            result,
            Ok((
                "",
                InvokeFunc {
                    name: "version".to_string(),
                    args: vec![]
                }
            ))
        );
    }

    #[tokio::test]
    async fn test_parse_invoke_list() {
        let query = "version()";
        let result = invoke_list(query);
        assert_eq!(
            result,
            Ok((
                "",
                vec![InvokeFunc {
                    name: "version".to_string(),
                    args: vec![]
                }]
            ))
        )
    }

    #[tokio::test]
    async fn test_parse_invoke_list_many_invoke() {
        let query = "version(), hello()";
        let result = invoke_list(query);
        assert_eq!(
            result,
            Ok((
                "",
                vec![
                    InvokeFunc {
                        name: "version".to_string(),
                        args: vec![]
                    },
                    InvokeFunc {
                        name: "hello".to_string(),
                        args: vec![]
                    }
                ]
            ))
        )
    }

    #[tokio::test]
    async fn test_parse_select_from() {
        let query = "from name select version()";
        let result = select_from(query);
        assert_eq!(
            result,
            Ok((
                "",
                QueryExpr::SelectFrom {
                    from: "name".to_string(),
                    select: vec![InvokeFunc {
                        name: "version".to_string(),
                        args: vec![]
                    }]
                }
            ))
        )
    }

    #[tokio::test]
    async fn test_parse_query_select() {
        let query = "from name select version()";
        let result = parse_query(query).await;
        assert_eq!(
            result,
            Ok(QueryExpr::SelectFrom {
                from: "name".to_string(),
                select: vec![InvokeFunc {
                    name: "version".to_string(),
                    args: vec![]
                }]
            })
        );
    }
}
