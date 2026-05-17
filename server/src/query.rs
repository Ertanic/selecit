use crate::proto::client::LOGS_MANAGER;
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::{complete::take_while, tag},
    character::{
        char,
        complete::{space0, space1},
    },
    combinator::{map, map_opt},
    multi::separated_list0,
};

#[derive(Debug, PartialEq)]
pub struct QueryParseError;

#[derive(Debug, PartialEq)]
pub struct InvokeFuncArg {
    pub name: String,
    pub value: String,
}

#[derive(Debug, PartialEq)]
pub struct InvokeFunc {
    pub name: String,
    pub args: Vec<InvokeFuncArg>,
}

#[derive(Debug, PartialEq)]
pub enum QueryExpr {
    ListBy(String),
    SelectFrom { from: String, select: Vec<InvokeFunc> },
}

fn identifier(input: &str) -> IResult<&str, &str> {
    take_while(char::is_alphanumeric).parse(input)
}

fn field(input: &str) -> IResult<&str, &str> {
    take_while(char::is_alphabetic).parse(input)
}

fn list_by(input: &str) -> IResult<&str, QueryExpr> {
    let (input, _) = tag("list by").parse(input)?;
    let (input, _) = space1(input)?;
    let (input, field_name) = field(input)?;

    Ok((input, QueryExpr::ListBy(field_name.to_string())))
}

fn invoke_arg(input: &str) -> IResult<&str, InvokeFuncArg> {
    let (input, name) = field(input)?;

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
    let (input, name) = map(field, |s| s.to_string()).parse(input)?;
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
        let result = field(&query[8..]);
        assert_eq!(result, Ok(("", "name")));
    }

    #[tokio::test]
    async fn test_parse_list_by() {
        let query = "list by name";
        let result = tag::<_, _, nom::error::Error<&str>>("list by").parse(query);
        assert_eq!(result, Ok((" name", "list by")));
    }

    #[tokio::test]
    async fn test_parse_list_by_parser() {
        let query = "list by name";
        let result = list_by(query);
        assert_eq!(result, Ok(("", QueryExpr::ListBy("name".to_string()))));
    }

    #[tokio::test]
    async fn test_parse_query_list_by_name() {
        let query = "list by name";
        let result = parse_query(query).await;
        assert_eq!(result, Ok(QueryExpr::ListBy("name".to_string())));
    }

    #[tokio::test]
    async fn test_parse_identifier_parser() {
        let query = "from name select version()";
        let (rest, _) = tag::<_, _, nom::error::Error<&str>>("from").parse(query).unwrap();
        let (rest, _) = space1::<_, nom::error::Error<&str>>(rest).unwrap();

        let result = identifier(rest);

        assert_eq!(result, Ok((" select version()", "name")));
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
