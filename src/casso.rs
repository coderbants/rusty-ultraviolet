//! Cleanroom Rust port of upstream Go source files: `internal/casso/math.go`, `internal/casso/solver.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! A Cassowary constraint solver, ported from `github.com/lithdew/casso` with
//! the unused API removed. Used by the layout engine to resolve competing
//! segment sizes. The port is exact and deterministic: `Symbol`s are
//! monotonically assigned from an atomic counter, and the simplex iterations
//! follow the upstream order.
//! </public-docs>

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// The kind of a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// External symbols represent user-facing variables.
    External,
    /// Slack symbols back inequalities.
    Slack,
    /// Error symbols relax constraints below the required priority.
    Error,
    /// Dummy symbols back required equalities.
    Dummy,
}

impl SymbolKind {
    /// Returns whether the symbol kind is restricted (slack or error).
    pub fn restricted(&self) -> bool {
        matches!(self, SymbolKind::Slack | SymbolKind::Error)
    }
}

/// Symbol is an opaque identifier for a variable in the solver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(u64);

/// The zero symbol.
pub const ZERO: Symbol = Symbol(0);

static COUNT: AtomicU64 = AtomicU64::new(0);

impl Symbol {
    /// New creates a new external symbol.
    pub fn new() -> Symbol {
        next(SymbolKind::External)
    }

    /// T creates a [Term] with the given coefficient.
    pub fn t(self, coeff: f64) -> Term {
        Term { coeff, id: self }
    }

    /// Returns the kind of the symbol.
    pub fn kind(&self) -> SymbolKind {
        match self.0 >> 62 {
            0 => SymbolKind::External,
            1 => SymbolKind::Slack,
            2 => SymbolKind::Error,
            _ => SymbolKind::Dummy,
        }
    }

    /// Returns whether the symbol is the zero symbol.
    pub fn is_zero(&self) -> bool {
        *self == ZERO
    }

    /// Returns whether the symbol is restricted.
    pub fn restricted(&self) -> bool {
        !self.is_zero() && self.kind().restricted()
    }

    /// Returns whether the symbol is external.
    pub fn external(&self) -> bool {
        !self.is_zero() && self.kind() == SymbolKind::External
    }

    /// Returns whether the symbol is a dummy.
    pub fn is_dummy(&self) -> bool {
        !self.is_zero() && self.kind() == SymbolKind::Dummy
    }
}

impl Default for Symbol {
    fn default() -> Self {
        ZERO
    }
}

fn next(typ: SymbolKind) -> Symbol {
    // Go's `atomic.AddUint64` returns the *new* value, hence the +1.
    let n = COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    Symbol((n & 0x3fffffffffffffff) | ((typ as u64) << 62))
}

/// Priority represents the strength of a constraint.
pub type Priority = f64;

/// The priority of required constraints.
pub const REQUIRED: Priority = 1e9;

/// Op represents a constraint operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Equality.
    EQ,
    /// Greater than or equal.
    GTE,
    /// Less than or equal.
    LTE,
}

/// Constraint represents a linear constraint.
#[derive(Debug, Clone)]
pub struct Constraint {
    op: Op,
    expr: Expr,
}

impl Constraint {
    /// NewConstraint creates a new constraint.
    pub fn new_constraint(op: Op, constant: f64, terms: &[Term]) -> Constraint {
        Constraint {
            op,
            expr: new_expr(constant, terms),
        }
    }

    /// Returns the operator of the constraint.
    pub fn op(&self) -> Op {
        self.op
    }

    fn clone_c(&self) -> Constraint {
        Constraint {
            op: self.op,
            expr: self.expr.clone(),
        }
    }
}

/// Term represents a variable with a coefficient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Term {
    coeff: f64,
    id: Symbol,
}

/// A linear expression: a constant plus a set of terms.
#[derive(Debug, Clone, Default)]
pub struct Expr {
    constant: f64,
    terms: Vec<Term>,
}

fn new_expr(constant: f64, terms: &[Term]) -> Expr {
    Expr {
        constant,
        terms: terms.to_vec(),
    }
}

impl Expr {
    fn clone_expr(&self) -> Expr {
        Expr {
            constant: self.constant,
            terms: self.terms.clone(),
        }
    }

    fn find(&self, id: Symbol) -> Option<usize> {
        self.terms.iter().position(|t| t.id == id)
    }

    fn delete(&mut self, idx: usize) {
        self.terms.remove(idx);
    }

    fn add_symbol(&mut self, coeff: f64, id: Symbol) {
        if let Some(idx) = self.find(id) {
            self.terms[idx].coeff += coeff;
            if eqz(self.terms[idx].coeff) {
                self.delete(idx);
            }
            return;
        }
        if !eqz(coeff) {
            self.terms.push(Term { coeff, id });
        }
    }

    fn add_expr(&mut self, coeff: f64, other: &Expr) {
        self.constant += coeff * other.constant;
        for t in &other.terms {
            self.add_symbol(coeff * t.coeff, t.id);
        }
    }

    fn negate(&mut self) {
        self.constant = -self.constant;
        for t in &mut self.terms {
            t.coeff = -t.coeff;
        }
    }

    fn solve_for(&mut self, id: Symbol) {
        let Some(idx) = self.find(id) else {
            return;
        };
        let coeff = -1.0 / self.terms[idx].coeff;
        self.delete(idx);
        if coeff == 1.0 {
            return;
        }
        self.constant *= coeff;
        for t in &mut self.terms {
            t.coeff *= coeff;
        }
    }

    fn solve_for_symbols(&mut self, lhs: Symbol, rhs: Symbol) {
        self.add_symbol(-1.0, lhs);
        self.solve_for(rhs);
    }

    fn substitute(&mut self, id: Symbol, other: &Expr) {
        let Some(idx) = self.find(id) else {
            return;
        };
        let coeff = self.terms[idx].coeff;
        self.delete(idx);
        self.add_expr(coeff, other);
    }
}

fn eqz(val: f64) -> bool {
    if val < 0.0 {
        -val < 1.0e-8
    } else {
        val < 1.0e-8
    }
}

/// An error returned by the solver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolverError {
    /// The constraint is unsatisfiable.
    Unsatisfiable,
    /// The constraint references a nil symbol.
    BadTerm,
}

impl std::fmt::Display for SolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SolverError::Unsatisfiable => write!(f, "casso: constraint is unsatisfiable"),
            SolverError::BadTerm => write!(f, "casso: term references a nil symbol"),
        }
    }
}

impl std::error::Error for SolverError {}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Tag {
    #[allow(dead_code)]
    priority: Priority,
    marker: Symbol,
    other: Symbol,
}

/// Solver implements the Cassowary constraint solving algorithm.
#[derive(Debug, Default)]
pub struct Solver {
    tabs: HashMap<Symbol, Constraint>,
    tags: HashMap<Symbol, Tag>,
    infeasible: Vec<Symbol>,
    objective: Expr,
    artificial: Expr,
}

/// NewSolver creates a new constraint solver.
pub fn new_solver() -> Solver {
    Solver {
        tabs: HashMap::new(),
        tags: HashMap::new(),
        infeasible: Vec::new(),
        objective: Expr::default(),
        artificial: Expr::default(),
    }
}

impl Solver {
    /// Val returns the current value of a symbol.
    pub fn val(&self, id: Symbol) -> f64 {
        self.tabs
            .get(&id)
            .map_or(0.0, |row| row.expr.constant)
    }

    /// Add adds a constraint to the solver with the given priority.
    ///
    /// Returns the marker symbol of the added constraint on success.
    pub fn add(&mut self, priority: Priority, cell: Constraint) -> Result<Symbol, SolverError> {
        let t = Tag {
            priority,
            marker: ZERO,
            other: ZERO,
        };

        let mut c = cell.clone_c();
        c.expr.terms = Vec::with_capacity(cell.expr.terms.len());

        for term in &cell.expr.terms {
            if eqz(term.coeff) {
                continue;
            }
            if term.id.is_zero() {
                return Err(SolverError::BadTerm);
            }
            let resolved = self.tabs.get(&term.id).cloned();
            match resolved {
                None => c.expr.add_symbol(term.coeff, term.id),
                Some(expr) => c.expr.add_expr(term.coeff, &expr.expr),
            }
        }

        let mut t = t;
        match c.op {
            Op::LTE | Op::GTE => {
                let mut coeff = 1.0;
                if c.op == Op::GTE {
                    coeff = -1.0;
                }
                t.marker = next(SymbolKind::Slack);
                c.expr.add_symbol(coeff, t.marker);
                if priority < REQUIRED {
                    t.other = next(SymbolKind::Error);
                    c.expr.add_symbol(-coeff, t.other);
                    self.objective.add_symbol(priority, t.other);
                }
            }
            Op::EQ => {
                if priority < REQUIRED {
                    t.marker = next(SymbolKind::Error);
                    t.other = next(SymbolKind::Error);
                    c.expr.add_symbol(-1.0, t.marker);
                    c.expr.add_symbol(1.0, t.other);
                    self.objective.add_symbol(priority, t.marker);
                    self.objective.add_symbol(priority, t.other);
                } else {
                    t.marker = next(SymbolKind::Dummy);
                    c.expr.add_symbol(1.0, t.marker);
                }
            }
        }

        if c.expr.constant < 0.0 {
            c.expr.negate();
        }

        let subject = self.find_subject(&c, t)?;

        if subject.is_zero() {
            self.augment_artificial_variable(&c)?;
        } else {
            c.expr.solve_for(subject);
            self.substitute(subject, &c.expr);
            self.tabs.insert(subject, c);
        }

        self.tags.insert(t.marker, t);

        self.optimize_against(WhichExpr::Objective)?;
        Ok(t.marker)
    }

    fn find_subject(&self, cell: &Constraint, t: Tag) -> Result<Symbol, SolverError> {
        for term in &cell.expr.terms {
            if term.id.external() {
                return Ok(term.id);
            }
        }

        if t.marker.restricted() {
            if let Some(idx) = cell.expr.find(t.marker) {
                if cell.expr.terms[idx].coeff < 0.0 {
                    return Ok(t.marker);
                }
            }
        }

        if t.other.restricted() {
            if let Some(idx) = cell.expr.find(t.other) {
                if cell.expr.terms[idx].coeff < 0.0 {
                    return Ok(t.other);
                }
            }
        }

        for term in &cell.expr.terms {
            if !term.id.is_dummy() {
                return Ok(ZERO);
            }
        }

        if !eqz(cell.expr.constant) {
            return Err(SolverError::Unsatisfiable);
        }

        Ok(t.marker)
    }

    fn substitute(&mut self, id: Symbol, e: &Expr) {
        substitute_into(
            &mut self.tabs,
            &mut self.infeasible,
            &mut self.objective,
            &mut self.artificial,
            id,
            e,
        );
    }

    fn optimize_against(&mut self, which: WhichExpr) -> Result<(), SolverError> {
        match which {
            WhichExpr::Objective => optimize(
                &mut self.tabs,
                &mut self.infeasible,
                &mut self.objective,
                &mut self.artificial,
            ),
            WhichExpr::Artificial => optimize(
                &mut self.tabs,
                &mut self.infeasible,
                &mut self.artificial,
                &mut self.objective,
            ),
        }
    }

    fn augment_artificial_variable(&mut self, row: &Constraint) -> Result<(), SolverError> {
        let art = next(SymbolKind::Slack);

        self.tabs.insert(art, row.clone_c());
        self.artificial = row.expr.clone_expr();

        self.optimize_against(WhichExpr::Artificial)?;

        let success = eqz(self.artificial.constant);
        self.artificial = Expr::default();

        let artificial = self.tabs.remove(&art);
        if let Some(mut artificial) = artificial {
            if artificial.expr.terms.is_empty() {
                return Ok(());
            }

            let mut entry = ZERO;
            for term in &artificial.expr.terms {
                if term.id.restricted() {
                    entry = term.id;
                    break;
                }
            }
            if entry.is_zero() {
                return Err(SolverError::Unsatisfiable);
            }

            artificial.expr.solve_for_symbols(art, entry);
            self.substitute(entry, &artificial.expr);
            self.tabs.insert(entry, artificial);
        }

        let symbols: Vec<Symbol> = self.tabs.keys().cloned().collect();
        for symbol in symbols {
            let mut row = self.tabs.remove(&symbol).unwrap();
            if let Some(idx) = row.expr.find(art) {
                row.expr.delete(idx);
            }
            self.tabs.insert(symbol, row);
        }

        if let Some(idx) = self.objective.find(art) {
            self.objective.delete(idx);
        }

        if !success {
            return Err(SolverError::Unsatisfiable);
        }
        Ok(())
    }
}

/// Selects which objective expression [Solver::optimize_against] operates
/// on. Mirrors passing `&s.objective` or `&s.artificial` upstream.
enum WhichExpr {
    Objective,
    Artificial,
}

/// substitute replaces every occurrence of the given symbol in the table and
/// in both objective expressions, mirroring `Solver.substitute` upstream.
fn substitute_into(
    tabs: &mut HashMap<Symbol, Constraint>,
    infeasible: &mut Vec<Symbol>,
    objective: &mut Expr,
    artificial: &mut Expr,
    id: Symbol,
    e: &Expr,
) {
    let symbols: Vec<Symbol> = tabs.keys().cloned().collect();
    for symbol in symbols {
        let mut row = tabs.remove(&symbol).unwrap();
        row.expr.substitute(id, e);
        let constant = row.expr.constant;
        tabs.insert(symbol, row);
        if symbol.external() || constant >= 0.0 {
            continue;
        }
        infeasible.push(symbol);
    }
    objective.substitute(id, e);
    artificial.substitute(id, e);
}

/// optimize implements the simplex iteration loop, mirroring
/// `Solver.optimizeAgainst` upstream. The `objective` and `artificial`
/// parameters are swapped by the caller so that both expressions are always
/// updated, as the Go method does through its receiver.
fn optimize(
    tabs: &mut HashMap<Symbol, Constraint>,
    infeasible: &mut Vec<Symbol>,
    objective: &mut Expr,
    artificial: &mut Expr,
) -> Result<(), SolverError> {
    loop {
        let mut entry = ZERO;
        let mut exit = ZERO;

        for term in &objective.terms {
            if !term.id.is_dummy() && term.coeff < 0.0 {
                entry = term.id;
                break;
            }
        }
        if entry.is_zero() {
            return Ok(());
        }

        let mut ratio = f64::MAX;

        let symbols: Vec<Symbol> = tabs.keys().cloned().collect();
        for symbol in symbols {
            if symbol.external() {
                continue;
            }
            let row = &tabs[&symbol];
            let Some(idx) = row.expr.find(entry) else {
                continue;
            };
            let coeff = row.expr.terms[idx].coeff;
            if coeff >= 0.0 {
                continue;
            }
            let r = -row.expr.constant / coeff;
            if r < ratio {
                ratio = r;
                exit = symbol;
            }
        }

        let mut row = tabs.remove(&exit).unwrap();
        row.expr.solve_for_symbols(exit, entry);
        substitute_into(tabs, infeasible, objective, artificial, entry, &row.expr);
        tabs.insert(entry, row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_eq_f64(expected: f64, actual: f64) {
        assert!(
            (expected - actual).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn test_symbol() {
        let v = next(SymbolKind::External);
        assert!(!v.is_zero());
        assert_eq!(v.kind(), SymbolKind::External);

        let v = next(SymbolKind::Slack);
        assert!(!v.is_zero());
        assert_eq!(v.kind(), SymbolKind::Slack);

        let v = next(SymbolKind::Error);
        assert!(!v.is_zero());
        assert_eq!(v.kind(), SymbolKind::Error);

        let v = next(SymbolKind::Dummy);
        assert!(!v.is_zero());
        assert_eq!(v.kind(), SymbolKind::Dummy);
    }

    #[test]
    fn test_constraint() {
        let l = Symbol::new();
        let m = Symbol::new();
        let r = Symbol::new();

        let a = Constraint::new_constraint(
            Op::EQ,
            0.0,
            &[r.t(1.0), l.t(1.0), m.t(-2.0)],
        );
        let b = Constraint::new_constraint(Op::GTE, -100.0, &[r.t(1.0), l.t(-1.0)]);
        let c = Constraint::new_constraint(Op::GTE, 0.0, &[l.t(1.0)]);

        let mut s = new_solver();

        s.add(1e9, a).unwrap();
        s.add(1e9, b).unwrap();
        s.add(1e9, c).unwrap();

        assert_eq_f64(0.0, s.val(l));
        assert_eq_f64(50.0, s.val(m));
        assert_eq_f64(100.0, s.val(r));
    }

    #[test]
    fn test_constraint_requiring_artificial_variable() {
        let mut s = new_solver();

        let p1 = Symbol::new();
        let p2 = Symbol::new();
        let p3 = Symbol::new();
        let container = Symbol::new();

        s.add(1e9, Constraint::new_constraint(Op::EQ, -100.0, &[container.t(1.0)]))
            .unwrap();
        s.add(1e6, Constraint::new_constraint(Op::GTE, -30.0, &[p1.t(1.0)]))
            .unwrap();
        s.add(
            1e3,
            Constraint::new_constraint(Op::EQ, 0.0, &[p1.t(1.0), p3.t(-1.0)]),
        )
        .unwrap();
        s.add(
            1e9,
            Constraint::new_constraint(Op::EQ, 0.0, &[p2.t(1.0), p1.t(-2.0)]),
        )
        .unwrap();
        s.add(
            1e9,
            Constraint::new_constraint(
                Op::EQ,
                0.0,
                &[container.t(1.0), p1.t(-1.0), p2.t(-1.0), p3.t(-1.0)],
            ),
        )
        .unwrap();

        assert_eq_f64(30.0, s.val(p1));
        assert_eq_f64(60.0, s.val(p2));
        assert_eq_f64(10.0, s.val(p3));
        assert_eq_f64(100.0, s.val(container));
    }
}
