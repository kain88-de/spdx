use crate::LicenseReq;
use smallvec::SmallVec;
use std::fmt;

/// A license requirement inside an SPDX license expression, including
/// the span in the expression where it is located
#[derive(Debug, Clone)]
pub struct ExpressionReq {
    pub req: LicenseReq,
    pub span: std::ops::Range<u32>,
}

impl PartialEq for ExpressionReq {
    fn eq(&self, o: &Self) -> bool {
        self.req == o.req
    }
}

/// The joining operators supported by SPDX 2.1
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub enum Operator {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ExprNode {
    Op(Operator),
    Req(ExpressionReq),
}

/// An SPDX license expression that is both syntactically
/// and semantically valid, and can be evaluated
#[derive(Clone)]
pub struct Expression {
    pub(crate) expr: SmallVec<[ExprNode; 5]>,
    // We keep the original string around for display purposes only
    pub(crate) original: String,
}

impl Expression {
    /// Returns each of the license requirements in the license expression,
    /// but not the operators that join them together
    pub fn requirements(&self) -> impl Iterator<Item = &ExpressionReq> {
        self.expr.iter().filter_map(|item| match item {
            ExprNode::Req(req) => Some(req),
            _ => None,
        })
    }

    /// Evaluates the expression, using the provided function
    /// to determine if the licensee meets the requirements
    /// for each license term. If enough requirements are
    /// satisfied the evaluation will return true.
    pub fn evaluate<AF: FnMut(&LicenseReq) -> bool>(&self, mut allow_func: AF) -> bool {
        let mut result_stack = SmallVec::<[bool; 8]>::new();

        // We store the expression as postfix, so just evaluate each license
        // requirement in the order it comes, and then combining the previous
        // results according to each operator as it comes
        for node in self.expr.iter() {
            match node {
                ExprNode::Req(req) => {
                    let allowed = allow_func(&req.req);
                    result_stack.push(allowed);
                }
                ExprNode::Op(Operator::Or) => {
                    let a = result_stack.pop().unwrap();
                    let b = result_stack.pop().unwrap();

                    result_stack.push(a || b);
                }
                ExprNode::Op(Operator::And) => {
                    let a = result_stack.pop().unwrap();
                    let b = result_stack.pop().unwrap();

                    result_stack.push(a && b);
                }
            }
        }

        result_stack.pop().unwrap()
    }

    /// Just as with evaluate, the license expression is evaluated to see if
    /// enough license requirements in the expresssion are met for the evaluation
    /// to succeed, except this method also keeps track of each failed requirement
    /// and returns them, allowing for more detailed error reporting about precisely
    /// what terms in the expression caused the overall failure
    pub fn evaluate_with_failures<AF: FnMut(&LicenseReq) -> bool>(
        &self,
        mut allow_func: AF,
    ) -> Result<(), Vec<&ExpressionReq>> {
        let mut result_stack = SmallVec::<[bool; 8]>::new();
        let mut failures = Vec::new();

        // We store the expression as postfix, so just evaluate each license
        // requirement in the order it comes, and then combining the previous
        // results according to each operator as it comes
        for node in self.expr.iter() {
            match node {
                ExprNode::Req(req) => {
                    let allowed = allow_func(&req.req);
                    result_stack.push(allowed);

                    if !allowed {
                        failures.push(req);
                    }
                }
                ExprNode::Op(Operator::Or) => {
                    let a = result_stack.pop().unwrap();
                    let b = result_stack.pop().unwrap();

                    result_stack.push(a || b);
                }
                ExprNode::Op(Operator::And) => {
                    let a = result_stack.pop().unwrap();
                    let b = result_stack.pop().unwrap();

                    result_stack.push(a && b);
                }
            }
        }

        if let Some(false) = result_stack.pop() {
            Err(failures)
        } else {
            Ok(())
        }
    }
}

impl AsRef<str> for Expression {
    fn as_ref(&self) -> &str {
        &self.original
    }
}

impl fmt::Debug for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, node) in self.expr.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }

            match node {
                ExprNode::Req(req) => write!(f, "{}", req.req)?,
                ExprNode::Op(Operator::And) => f.write_str("AND")?,
                ExprNode::Op(Operator::Or) => f.write_str("OR")?,
            }
        }

        Ok(())
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.original)
    }
}

impl PartialEq for Expression {
    fn eq(&self, o: &Self) -> bool {
        // The expressions can be semantically the same but not
        // syntactically the same, if the user wants to compare
        // the raw expressions they can just do a string compare
        if self.expr.len() != o.expr.len() {
            return false;
        }

        !self.expr.iter().zip(o.expr.iter()).any(|(a, b)| a != b)
    }
}

#[cfg(test)]
mod test {
    use super::Expression;

    #[test]
    fn eq() {
        let normal = Expression::parse("MIT OR Apache-2.0").unwrap();
        let extra_parens = Expression::parse("(MIT OR (Apache-2.0))").unwrap();
        let llvm_exc = Expression::parse("MIT OR Apache-2.0 WITH LLVM-exception").unwrap();

        assert_eq!(normal, normal);
        assert_eq!(extra_parens, extra_parens);
        assert_eq!(llvm_exc, llvm_exc);

        assert_eq!(normal, extra_parens);

        assert_ne!(normal, llvm_exc);
    }
}
