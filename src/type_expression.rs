// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use alloc::{format, string::ToString, vec};
use core::ops::Deref;
use sql_parse::{issue_todo, Expression, Issue, Span, UnaryOperator};

use crate::{
    schema::parse_column,
    type_::{BaseType, FullType},
    type_binary_expression::type_binary_expression,
    type_function::type_function,
    type_select::{resolve_kleene_identifier, type_select},
    typer::Typer,
    Type,
};

fn type_unary_expression<'a, 'b>(
    typer: &mut Typer<'a, 'b>,
    op: &UnaryOperator,
    op_span: &Span,
    operand: &Expression<'a>,
) -> FullType<'a> {
    let op_type = type_expression(typer, operand, false);
    match op {
        UnaryOperator::Binary
        | UnaryOperator::Collate
        | UnaryOperator::LogicalNot
        | UnaryOperator::Minus => {
            typer.issues.push(issue_todo!(op_span));
            FullType::invalid()
        }
        UnaryOperator::Not => {
            typer.ensure_base(operand, &op_type, BaseType::Bool);
            op_type
        }
    }
}

pub(crate) fn type_expression<'a, 'b>(
    typer: &mut Typer<'a, 'b>,
    expression: &Expression<'a>,
    outer_where: bool,
) -> FullType<'a> {
    match expression {
        Expression::Binary {
            op,
            op_span,
            lhs,
            rhs,
        } => type_binary_expression(typer, op, op_span, lhs, rhs, outer_where),
        Expression::Unary {
            op,
            op_span,
            operand,
        } => type_unary_expression(typer, op, op_span, operand),
        Expression::Subquery(select) => {
            let select_type = type_select(typer, select, false);
            if let [v] = select_type.columns.as_slice() {
                v.type_.clone()
            } else {
                typer
                    .issues
                    .push(Issue::err("Subquery should yield one column", select));
                FullType::invalid()
            }
        }
        Expression::Null(_) => FullType::new(Type::Null, false),
        Expression::Bool(_, _) => FullType::new(BaseType::Bool, true),
        Expression::String(_) => FullType::new(BaseType::String, true),
        Expression::Integer(_) => FullType::new(BaseType::Integer, true),
        Expression::Float(_) => FullType::new(BaseType::Float, true),
        Expression::Function(func, args, span) => type_function(typer, func, args, span),
        Expression::Identifier(i) => {
            let mut t = None;
            match i.as_slice() {
                [part] => {
                    let col = match part {
                        sql_parse::IdentifierPart::Name(n) => n,
                        sql_parse::IdentifierPart::Star(v) => {
                            typer.issues.push(Issue::err("Not supported here", v));
                            return FullType::invalid();
                        }
                    };
                    let mut cnt = 0;
                    for r in &typer.reference_types {
                        for c in &r.columns {
                            if c.0 == col.value {
                                cnt += 1;
                                t = Some(c);
                            }
                        }
                    }
                    if cnt > 1 {
                        let mut issue = Issue::err("Ambigious reference", col);
                        for r in &typer.reference_types {
                            for c in &r.columns {
                                if c.0 == col.value {
                                    issue = issue.frag("Defined here", &r.span);
                                }
                            }
                        }
                        typer.issues.push(issue);
                        return FullType::invalid();
                    }
                }
                [p1, p2] => {
                    let tbl = match p1 {
                        sql_parse::IdentifierPart::Name(n) => n,
                        sql_parse::IdentifierPart::Star(v) => {
                            typer.issues.push(Issue::err("Not supported here", v));
                            return FullType::invalid();
                        }
                    };
                    let col = match p2 {
                        sql_parse::IdentifierPart::Name(n) => n,
                        sql_parse::IdentifierPart::Star(v) => {
                            typer.issues.push(Issue::err("Not supported here", v));
                            return FullType::invalid();
                        }
                    };
                    for r in &typer.reference_types {
                        if r.name == Some(tbl.value) {
                            for c in &r.columns {
                                if c.0 == col.value {
                                    t = Some(c);
                                }
                            }
                        }
                    }
                }
                _ => {
                    typer
                        .issues
                        .push(Issue::err("Bad identifier length", expression));
                    return FullType::invalid();
                }
            }
            match t {
                None => {
                    typer
                        .issues
                        .push(Issue::err("Unknown identifier", expression));
                    FullType::invalid()
                }
                Some((_, type_)) => type_.clone(),
            }
        }
        Expression::Arg((idx, span)) => {
            FullType::new(Type::Args(BaseType::Any, vec![(*idx, span.clone())]), false)
        }
        Expression::Exists(s) => {
            type_select(typer, s, false);
            FullType::new(BaseType::Bool, true)
        }
        Expression::In {
            lhs, rhs, in_span, ..
        } => {
            let mut lhs_type = type_expression(typer, lhs, false);
            let mut not_null = lhs_type.not_null;
            // Hack to allow null arguments on the right hand side of an in expression
            // where the lhs is not null
            lhs_type.not_null = false;
            for rhs in rhs {
                let rhs_type = if let Expression::Subquery(q) = rhs {
                    let rhs_type = type_select(typer, q, false);
                    if rhs_type.columns.len() != 1 {
                        typer.issues.push(Issue::err(
                            format!(
                                "Subquery in IN should yield one column but gave {}",
                                rhs_type.columns.len()
                            ),
                            q,
                        ))
                    }
                    if let Some(c) = rhs_type.columns.get(0) {
                        c.type_.clone()
                    } else {
                        FullType::invalid()
                    }
                } else {
                    type_expression(typer, rhs, false)
                };
                not_null &= rhs_type.not_null;
                if typer.matched_type(&lhs_type, &rhs_type).is_none() {
                    typer.issues.push(
                        Issue::err("Incompatible types", in_span)
                            .frag(lhs_type.t.to_string(), lhs)
                            .frag(rhs_type.to_string(), rhs),
                    );
                }
            }
            FullType::new(BaseType::Bool, not_null)
        }
        Expression::Is(e, is, _) => {
            let t = type_expression(typer, e, false);
            match is {
                sql_parse::Is::Null => {
                    if t.not_null {
                        typer.issues.push(Issue::warn("Cannot be null", e));
                    }
                    FullType::new(BaseType::Bool, true)
                }
                sql_parse::Is::NotNull => {
                    if t.not_null {
                        typer.issues.push(Issue::warn("Cannot be null", e));
                    }
                    if outer_where {
                        // If were are in the outer part of a where expression possibly behind ands,
                        // and the expression is an identifier, we can mark the columns not_null
                        // the reference_types
                        if let Expression::Identifier(parts) = e.as_ref() {
                            if let Some(sql_parse::IdentifierPart::Name(n0)) = parts.get(0) {
                                if parts.len() == 1 {
                                    for r in &mut typer.reference_types {
                                        for c in &mut r.columns {
                                            if c.0 == n0.value {
                                                c.1.not_null = true;
                                            }
                                        }
                                    }
                                } else if let Some(sql_parse::IdentifierPart::Name(n1)) =
                                    parts.get(1)
                                {
                                    for r in &mut typer.reference_types {
                                        if r.name == Some(n0.value) {
                                            for c in &mut r.columns {
                                                if c.0 == n1.value {
                                                    c.1.not_null = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    FullType::new(BaseType::Bool, true)
                }
                sql_parse::Is::True
                | sql_parse::Is::NotTrue
                | sql_parse::Is::False
                | sql_parse::Is::NotFalse
                | sql_parse::Is::Unknown
                | sql_parse::Is::NotUnknown => {
                    typer.issues.push(issue_todo!(expression));
                    FullType::invalid()
                }
            }
        }
        Expression::Invalid(_) => FullType::invalid(),
        Expression::Case { .. } => {
            typer.issues.push(issue_todo!(expression));
            FullType::invalid()
        }
        Expression::Cast {
            expr,
            as_span,
            type_,
            ..
        } => {
            let e = type_expression(typer, expr, false);
            let col = parse_column(type_.clone(), as_span.clone(), typer.issues);
            //TODO check if it can possible be valid cast
            FullType::new(col.type_.t, e.not_null)
        }
        Expression::Count { expr, .. } => {
            match expr.deref() {
                Expression::Identifier(parts) => {
                    resolve_kleene_identifier(typer, parts, &None, |_, _, _, _| {})
                }
                arg => {
                    type_expression(typer, arg, false);
                }
            }
            FullType::new(BaseType::Integer, true)
        }
    }
}
