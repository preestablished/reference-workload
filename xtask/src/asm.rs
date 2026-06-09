//! Two-pass 65C816 assembler for the synthetic test ROM.
//!
//! # Subset implemented
//!
//! ## Directives
//! - `.org ADDR` — set assembly origin (24-bit)
//! - `.db e1, e2, ...` — emit byte(s)
//! - `.dw e1, e2, ...` — emit little-endian u16 word(s)
//! - `.fill COUNT, BYTE` — fill COUNT bytes with BYTE
//! - `.a8 / .a16 / .i8 / .i16` — immediate-width state (affects instruction encoding)
//! - `.ascii "str"` — emit raw ASCII bytes (no NUL)
//!
//! ## Expressions
//! Literals: `$xx` hex, decimal; symbol references; binary `+ - * & | << >>`;
//! unary `<` (low byte), `>` (high byte), `^` (bank byte).
//!
//! ## Instructions
//! Full 65C816 mnemonic set needed by synth.s65, with standard encodings.
//! Addressing modes recognised: `#imm`, `dp`, `dp,X`, `dp,Y`, `addr`, `addr,X`,
//! `addr,Y`, `long`, `long,X`, `(dp,X)`, `(dp),Y`, `(dp)`, `[dp]`, `[dp],Y`,
//! `(addr)`, `(addr,X)`, `[long]`, `LABEL` (branch target). MVN/MVP use
//! `#bank,#bank` syntax.
//!
//! Unknown mnemonics produce a compile error with file:line.

#![allow(clippy::manual_strip)] // sigil parsing tests-then-slices throughout

use std::collections::BTreeMap;

// ─── public error type ────────────────────────────────────────────────────────

/// Assembler diagnostic.
#[derive(Debug, Clone)]
pub struct AsmError {
    pub line: usize,
    pub msg: String,
}

impl std::fmt::Display for AsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for AsmError {}

// ─── width state ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    W8,
    W16,
}

// ─── addressing modes ─────────────────────────────────────────────────────────

// ─── expression AST ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) enum Expr {
    Lit(i64),
    Sym(String),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Shl(Box<Expr>, Box<Expr>),
    Shr(Box<Expr>, Box<Expr>),
    LowByte(Box<Expr>),
    HighByte(Box<Expr>),
    BankByte(Box<Expr>),
    CurrentAddr, // * (current PC)
}

// ─── assembler state ──────────────────────────────────────────────────────────

pub struct Assembler {
    /// Symbol table: label / constant -> value
    symbols: BTreeMap<String, i64>,
}

impl Default for Assembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Assembler {
    pub fn new() -> Self {
        Assembler {
            symbols: BTreeMap::new(),
        }
    }

    /// Assemble source text into a flat byte buffer starting at `base_origin`.
    /// Returns (bytes, base_origin) where `bytes[i]` corresponds to address
    /// `base_origin + i as u32`.
    pub fn assemble(&mut self, src: &str) -> Result<(Vec<u8>, u32), AsmError> {
        self.assemble_two_pass(src)
    }

    /// Clean two-pass assembler. Pass1 sets symbols + computes sizes.
    /// Pass2 emits bytes into a Vec<u8> indexed from base_origin.
    fn assemble_two_pass(&mut self, src: &str) -> Result<(Vec<u8>, u32), AsmError> {
        // ── PASS 1 ──
        let mut pc: u32 = 0;
        let mut first_pc: Option<u32> = None;
        let mut last_pc: u32 = 0;
        let mut a_w = Width::W8;
        let mut i_w = Width::W8;

        let lines: Vec<String> = src
            .lines()
            .map(|l| strip_comment(l).trim().to_owned())
            .collect();

        for (idx, line) in lines.iter().enumerate() {
            let lineno = idx + 1;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let (rest, label_opt) = parse_label(line);
            if let Some(lbl) = label_opt {
                self.symbols.insert(lbl, pc as i64);
            }

            let rest = rest.trim();
            if rest.is_empty() {
                continue;
            }

            if let Some(pos) = find_assignment(rest) {
                let name = rest[..pos].trim().to_string();
                let expr_src = rest[pos + 1..].trim();
                let expr = self.parse_expr(expr_src, lineno)?;
                match self.eval_expr(&expr) {
                    Ok(v) => {
                        self.symbols.insert(name, v);
                    }
                    Err(_) => {
                        self.symbols.insert(name, 0);
                    } // forward ref placeholder
                }
                continue;
            }

            let sz = self.size_of(rest, pc, a_w, i_w, lineno)?;
            match sz {
                SizeResult::OrgSet(addr) => {
                    pc = addr;
                    if first_pc.is_none() {
                        first_pc = Some(addr);
                    }
                }
                SizeResult::AWidth(w) => {
                    a_w = w;
                }
                SizeResult::IWidth(w) => {
                    i_w = w;
                }
                SizeResult::Bytes(n) => {
                    if first_pc.is_none() {
                        first_pc = Some(pc);
                    }
                    pc = pc.wrapping_add(n as u32);
                    last_pc = pc;
                }
            }
        }

        // Second pass for constants with forward refs.
        for (idx, line) in lines.iter().enumerate() {
            let lineno = idx + 1;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let (rest, _) = parse_label(line);
            let rest = rest.trim();
            if rest.is_empty() {
                continue;
            }
            if let Some(pos) = find_assignment(rest) {
                let name = rest[..pos].trim().to_string();
                let expr_src = rest[pos + 1..].trim();
                let expr = self.parse_expr(expr_src, lineno)?;
                if let Ok(v) = self.eval_expr(&expr) {
                    self.symbols.insert(name, v);
                }
            }
        }

        let base = first_pc.unwrap_or(0);
        let end = last_pc;
        // Handle 64K wrap: if end <= base it may have wrapped around $10000.
        let size = if end > base {
            (end - base) as usize
        } else if end == 0 && base <= 0x8000 {
            // ROM fills exactly to $10000 (e.g., 32KB LoROM: base=$8000, end wraps to 0)
            (0x10000u32 - base) as usize
        } else if end < base {
            // Wrapped: total = ($10000 - base) + end
            (0x10000u32 - base + end) as usize
        } else {
            1
        };
        let mut buf = vec![0u8; size];

        // ── PASS 2: emit ──
        pc = base;
        a_w = Width::W8;
        i_w = Width::W8;

        for (idx, line) in lines.iter().enumerate() {
            let lineno = idx + 1;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let (rest, label_opt) = parse_label(line);
            // Guard against pass-1/pass-2 size disagreement: a label whose
            // pass-1 address differs from the actual emission address means
            // an instruction was sized wrong — fail loudly instead of
            // emitting a ROM with corrupt label references.
            if let Some(lbl) = label_opt {
                let want = self.symbols.get(&lbl).copied().unwrap_or(-1);
                if want != pc as i64 {
                    return Err(AsmError {
                        line: lineno,
                        msg: format!(
                            "label `{lbl}`: pass-1 address ${want:04X} != \
                             pass-2 emission address ${pc:04X} \
                             (assembler sizing bug)"
                        ),
                    });
                }
            }
            let rest = rest.trim();
            if rest.is_empty() {
                continue;
            }

            if find_assignment(rest).is_some() {
                continue;
            }

            match self.emit(rest, pc, a_w, i_w, lineno)? {
                EmitResult::OrgSet(addr) => {
                    pc = addr;
                }
                EmitResult::AWidth(w) => {
                    a_w = w;
                }
                EmitResult::IWidth(w) => {
                    i_w = w;
                }
                EmitResult::Bytes(bytes) => {
                    for (i, b) in bytes.iter().enumerate() {
                        let off = (pc as usize).wrapping_sub(base as usize) + i;
                        if off < buf.len() {
                            buf[off] = *b;
                        }
                    }
                    pc = pc.wrapping_add(bytes.len() as u32);
                }
            }
        }

        Ok((buf, base))
    }
}

// ─── size_of / emit helpers ───────────────────────────────────────────────────

enum SizeResult {
    OrgSet(u32),
    AWidth(Width),
    IWidth(Width),
    Bytes(usize),
}

enum EmitResult {
    OrgSet(u32),
    AWidth(Width),
    IWidth(Width),
    Bytes(Vec<u8>),
}

impl Assembler {
    fn size_of(
        &mut self,
        rest: &str,
        pc: u32,
        a_w: Width,
        i_w: Width,
        lineno: usize,
    ) -> Result<SizeResult, AsmError> {
        // try directive
        if let Some(sr) = self.try_directive_size(rest, pc, a_w, i_w, lineno)? {
            return Ok(sr);
        }
        // try instruction
        let size = self.instr_size(rest, a_w, i_w, lineno)?;
        Ok(SizeResult::Bytes(size))
    }

    fn emit(
        &mut self,
        rest: &str,
        pc: u32,
        a_w: Width,
        i_w: Width,
        lineno: usize,
    ) -> Result<EmitResult, AsmError> {
        if let Some(er) = self.try_directive_emit(rest, pc, a_w, i_w, lineno)? {
            return Ok(er);
        }
        let bytes = self.emit_instr(rest, pc, a_w, i_w, lineno)?;
        Ok(EmitResult::Bytes(bytes))
    }

    // ── directive: size ──

    fn try_directive_size(
        &mut self,
        rest: &str,
        _pc: u32,
        _a_w: Width,
        _i_w: Width,
        lineno: usize,
    ) -> Result<Option<SizeResult>, AsmError> {
        let low = rest.to_ascii_lowercase();
        let low = low.trim();

        if low.starts_with(".org") {
            let arg = rest[4..].trim();
            let expr = self.parse_expr(arg, lineno)?;
            let addr = self.eval_expr(&expr).map_err(|e| AsmError {
                line: lineno,
                msg: format!(".org: {}", e),
            })? as u32;
            return Ok(Some(SizeResult::OrgSet(addr)));
        }

        if low.starts_with(".a16") {
            return Ok(Some(SizeResult::AWidth(Width::W16)));
        }
        if low.starts_with(".a8") {
            return Ok(Some(SizeResult::AWidth(Width::W8)));
        }
        if low.starts_with(".i16") {
            return Ok(Some(SizeResult::IWidth(Width::W16)));
        }
        if low.starts_with(".i8") {
            return Ok(Some(SizeResult::IWidth(Width::W8)));
        }

        if low.starts_with(".db") || low.starts_with(".byte") {
            let args = parse_list(&rest[3..]);
            return Ok(Some(SizeResult::Bytes(args.len())));
        }

        if low.starts_with(".dw") || low.starts_with(".word") {
            let args = parse_list(&rest[3..]);
            return Ok(Some(SizeResult::Bytes(args.len() * 2)));
        }

        if low.starts_with(".fill") {
            let (cnt, _byte) = parse_fill(&rest[5..], lineno, &mut |e| self.eval_expr(e))?;
            return Ok(Some(SizeResult::Bytes(cnt)));
        }

        if low.starts_with(".ascii") {
            let s = parse_string(rest[6..].trim(), lineno)?;
            return Ok(Some(SizeResult::Bytes(s.len())));
        }

        Ok(None)
    }

    // ── directive: emit ──

    fn try_directive_emit(
        &mut self,
        rest: &str,
        _pc: u32,
        _a_w: Width,
        _i_w: Width,
        lineno: usize,
    ) -> Result<Option<EmitResult>, AsmError> {
        let low = rest.to_ascii_lowercase();
        let low = low.trim();

        if low.starts_with(".org") {
            let arg = rest[4..].trim();
            let expr = self.parse_expr(arg, lineno)?;
            let addr = self.eval_expr(&expr).map_err(|e| AsmError {
                line: lineno,
                msg: format!(".org: {}", e),
            })? as u32;
            return Ok(Some(EmitResult::OrgSet(addr)));
        }

        if low.starts_with(".a16") {
            return Ok(Some(EmitResult::AWidth(Width::W16)));
        }
        if low.starts_with(".a8") {
            return Ok(Some(EmitResult::AWidth(Width::W8)));
        }
        if low.starts_with(".i16") {
            return Ok(Some(EmitResult::IWidth(Width::W16)));
        }
        if low.starts_with(".i8") {
            return Ok(Some(EmitResult::IWidth(Width::W8)));
        }

        if low.starts_with(".db") || low.starts_with(".byte") {
            let mut bytes = Vec::new();
            for tok in parse_list(&rest[3..]) {
                let expr = self.parse_expr(tok.trim(), lineno)?;
                let v = self.eval_expr(&expr).map_err(|e| AsmError {
                    line: lineno,
                    msg: format!(".db: {}", e),
                })?;
                bytes.push(v as u8);
            }
            return Ok(Some(EmitResult::Bytes(bytes)));
        }

        if low.starts_with(".dw") || low.starts_with(".word") {
            let mut bytes = Vec::new();
            for tok in parse_list(&rest[3..]) {
                let expr = self.parse_expr(tok.trim(), lineno)?;
                let v = self.eval_expr(&expr).map_err(|e| AsmError {
                    line: lineno,
                    msg: format!(".dw: {}", e),
                })? as u16;
                bytes.push(v as u8);
                bytes.push((v >> 8) as u8);
            }
            return Ok(Some(EmitResult::Bytes(bytes)));
        }

        if low.starts_with(".fill") {
            let (cnt, byte) = parse_fill(&rest[5..], lineno, &mut |e| self.eval_expr(e))?;
            return Ok(Some(EmitResult::Bytes(vec![byte; cnt])));
        }

        if low.starts_with(".ascii") {
            let s = parse_string(rest[6..].trim(), lineno)?;
            return Ok(Some(EmitResult::Bytes(s)));
        }

        Ok(None)
    }

    // ── instruction size ──

    fn instr_size(
        &self,
        rest: &str,
        a_w: Width,
        i_w: Width,
        lineno: usize,
    ) -> Result<usize, AsmError> {
        let (mne, _ops) = split_mnemonic(rest);
        let mne = mne.to_ascii_uppercase();
        // Parse the mode to determine size
        match instr_size_for(&mne, rest, a_w, i_w, self) {
            Some(s) => Ok(s),
            None => Err(AsmError {
                line: lineno,
                msg: format!("unknown mnemonic '{}'", mne),
            }),
        }
    }

    // ── instruction emit ──

    fn emit_instr(
        &mut self,
        rest: &str,
        pc: u32,
        a_w: Width,
        i_w: Width,
        lineno: usize,
    ) -> Result<Vec<u8>, AsmError> {
        let (mne, ops) = split_mnemonic(rest);
        let mne = mne.to_ascii_uppercase();
        encode_instruction(&mne, ops.trim(), pc, a_w, i_w, lineno, self)
    }
}

// ─── expression parser ────────────────────────────────────────────────────────

impl Assembler {
    fn parse_expr(&self, s: &str, lineno: usize) -> Result<Expr, AsmError> {
        let s = s.trim();
        parse_expr_str(s, lineno)
    }

    pub(crate) fn eval_expr(&self, e: &Expr) -> Result<i64, String> {
        match e {
            Expr::Lit(v) => Ok(*v),
            Expr::Sym(name) => self
                .symbols
                .get(name)
                .copied()
                .ok_or_else(|| format!("undefined symbol '{}'", name)),
            Expr::Add(a, b) => Ok(self.eval_expr(a)? + self.eval_expr(b)?),
            Expr::Sub(a, b) => Ok(self.eval_expr(a)? - self.eval_expr(b)?),
            Expr::Mul(a, b) => Ok(self.eval_expr(a)? * self.eval_expr(b)?),
            Expr::And(a, b) => Ok(self.eval_expr(a)? & self.eval_expr(b)?),
            Expr::Or(a, b) => Ok(self.eval_expr(a)? | self.eval_expr(b)?),
            Expr::Shl(a, b) => Ok(self.eval_expr(a)? << self.eval_expr(b)?),
            Expr::Shr(a, b) => Ok(self.eval_expr(a)? >> self.eval_expr(b)?),
            Expr::LowByte(a) => Ok(self.eval_expr(a)? & 0xFF),
            Expr::HighByte(a) => Ok((self.eval_expr(a)? >> 8) & 0xFF),
            Expr::BankByte(a) => Ok((self.eval_expr(a)? >> 16) & 0xFF),
            Expr::CurrentAddr => Err("* not supported in this context".into()),
        }
    }
}

// ─── expression parser (recursive descent) ────────────────────────────────────

fn parse_expr_str(s: &str, lineno: usize) -> Result<Expr, AsmError> {
    let s = s.trim();
    parse_or(s, lineno)
}

fn parse_or(s: &str, lineno: usize) -> Result<Expr, AsmError> {
    // Split on | (but not <<, >>)
    let s = s.trim();
    // find lowest-precedence split: + -
    parse_addsub(s, lineno)
}

fn parse_addsub(s: &str, lineno: usize) -> Result<Expr, AsmError> {
    let s = s.trim();
    // find rightmost + or - at depth 0 (outside parens)
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut split_pos: Option<(usize, char)> = None;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b'+' | b'-' if depth == 0 && i > 0 => {
                split_pos = Some((i, bytes[i] as char));
            }
            _ => {}
        }
        i += 1;
    }
    if let Some((pos, op)) = split_pos {
        let left = parse_muldiv(s[..pos].trim(), lineno)?;
        let right = parse_muldiv(s[pos + 1..].trim(), lineno)?;
        return Ok(if op == '+' {
            Expr::Add(Box::new(left), Box::new(right))
        } else {
            Expr::Sub(Box::new(left), Box::new(right))
        });
    }
    parse_muldiv(s, lineno)
}

fn parse_muldiv(s: &str, lineno: usize) -> Result<Expr, AsmError> {
    let s = s.trim();
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut split_pos: Option<(usize, char)> = None;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b'*' if depth == 0 && i > 0 => {
                split_pos = Some((i, '*'));
            }
            _ => {}
        }
        i += 1;
    }
    if let Some((pos, _)) = split_pos {
        let left = parse_bitops(s[..pos].trim(), lineno)?;
        let right = parse_bitops(s[pos + 1..].trim(), lineno)?;
        return Ok(Expr::Mul(Box::new(left), Box::new(right)));
    }
    parse_bitops(s, lineno)
}

fn parse_bitops(s: &str, lineno: usize) -> Result<Expr, AsmError> {
    let s = s.trim();
    // Check for << and >> first (before & and |)
    if let Some(pos) = find_outside_parens(s, "<<") {
        let left = parse_unary(s[..pos].trim(), lineno)?;
        let right = parse_unary(s[pos + 2..].trim(), lineno)?;
        return Ok(Expr::Shl(Box::new(left), Box::new(right)));
    }
    if let Some(pos) = find_outside_parens(s, ">>") {
        let left = parse_unary(s[..pos].trim(), lineno)?;
        let right = parse_unary(s[pos + 2..].trim(), lineno)?;
        return Ok(Expr::Shr(Box::new(left), Box::new(right)));
    }
    // & and |
    if let Some(pos) = find_char_outside_parens(s, '&') {
        let left = parse_unary(s[..pos].trim(), lineno)?;
        let right = parse_unary(s[pos + 1..].trim(), lineno)?;
        return Ok(Expr::And(Box::new(left), Box::new(right)));
    }
    if let Some(pos) = find_char_outside_parens(s, '|') {
        let left = parse_unary(s[..pos].trim(), lineno)?;
        let right = parse_unary(s[pos + 1..].trim(), lineno)?;
        return Ok(Expr::Or(Box::new(left), Box::new(right)));
    }
    parse_unary(s, lineno)
}

fn find_outside_parens(s: &str, pat: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let pbytes = pat.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i + pbytes.len() <= bytes.len() {
        match bytes[i] {
            b'(' | b'[' => {
                depth += 1;
                i += 1;
                continue;
            }
            b')' | b']' => {
                depth -= 1;
                i += 1;
                continue;
            }
            _ => {}
        }
        if depth == 0 && &bytes[i..i + pbytes.len()] == pbytes {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_char_outside_parens(s: &str, c: char) -> Option<usize> {
    let bytes = s.as_bytes();
    let cb = c as u8;
    let mut depth = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b if b == cb && depth == 0 => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

fn parse_unary(s: &str, lineno: usize) -> Result<Expr, AsmError> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('<') {
        return Ok(Expr::LowByte(Box::new(parse_primary(rest, lineno)?)));
    }
    if let Some(rest) = s.strip_prefix('>') {
        return Ok(Expr::HighByte(Box::new(parse_primary(rest, lineno)?)));
    }
    if let Some(rest) = s.strip_prefix('^') {
        return Ok(Expr::BankByte(Box::new(parse_primary(rest, lineno)?)));
    }
    parse_primary(s, lineno)
}

fn parse_primary(s: &str, lineno: usize) -> Result<Expr, AsmError> {
    let s = s.trim();
    if s == "*" {
        return Ok(Expr::CurrentAddr);
    }
    if s.starts_with('(') && s.ends_with(')') {
        return parse_expr_str(&s[1..s.len() - 1], lineno);
    }
    if let Some(hex) = s.strip_prefix('$') {
        return i64::from_str_radix(hex, 16)
            .map(Expr::Lit)
            .map_err(|_| AsmError {
                line: lineno,
                msg: format!("bad hex literal '{}'", s),
            });
    }
    if let Ok(v) = s.parse::<i64>() {
        return Ok(Expr::Lit(v));
    }
    // Symbol
    if is_identifier(s) {
        return Ok(Expr::Sym(s.to_string()));
    }
    Err(AsmError {
        line: lineno,
        msg: format!("cannot parse expression '{}'", s),
    })
}

fn is_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .enumerate()
            .all(|(i, c)| c.is_alphanumeric() || c == '_' || (i > 0 && c == '.'))
}

// ─── utility functions ────────────────────────────────────────────────────────

fn strip_comment(line: &str) -> &str {
    // Find ';' not inside a string literal
    let bytes = line.as_bytes();
    let mut in_str = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'"' => in_str = !in_str,
            b';' if !in_str => return &line[..i],
            _ => {}
        }
    }
    line
}

fn parse_label(line: &str) -> (&str, Option<String>) {
    // A label is identifier followed immediately by ':' (no space before ':')
    let line = line.trim();
    // Find first ':' that is not inside a string or after a '#'
    let bytes = line.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b':' {
            let candidate = &line[..i];
            if is_identifier(candidate.trim()) && !candidate.contains(' ') {
                return (&line[i + 1..], Some(candidate.trim().to_string()));
            }
            break;
        }
        // if we hit a space before ':', it might still be "label: instr"
        // but we only treat "label:" with no spaces in the label itself
        if b == b' ' || b == b'\t' {
            // check if what follows after space is ":"
            let rest = line[i..].trim_start();
            if rest.starts_with(':') {
                let candidate = &line[..i];
                if is_identifier(candidate.trim()) {
                    return (&rest[1..], Some(candidate.trim().to_string()));
                }
            }
            break;
        }
    }
    (line, None)
}

fn find_assignment(s: &str) -> Option<usize> {
    // Find '=' not part of '==' or preceded by '!'
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'=' {
            // make sure this is an assignment, not inside an expression
            // Simple heuristic: if lhs is identifier, treat as assignment
            let lhs = s[..i].trim();
            if is_identifier(lhs) {
                return Some(i);
            }
        }
    }
    None
}

fn parse_list(s: &str) -> Vec<&str> {
    // Split on commas, respecting parens/brackets and strings
    let s = s.trim();
    let bytes = s.as_bytes();
    let mut result = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    let mut in_str = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'"' => in_str = !in_str,
            b'(' | b'[' if !in_str => depth += 1,
            b')' | b']' if !in_str => depth -= 1,
            b',' if depth == 0 && !in_str => {
                result.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = s[start..].trim();
    if !last.is_empty() {
        result.push(last);
    }
    result
}

fn parse_fill<F>(s: &str, lineno: usize, eval: &mut F) -> Result<(usize, u8), AsmError>
where
    F: FnMut(&Expr) -> Result<i64, String>,
{
    let parts = parse_list(s);
    if parts.len() < 2 {
        return Err(AsmError {
            line: lineno,
            msg: ".fill requires two arguments (count, byte)".into(),
        });
    }
    let cnt_expr = parse_expr_str(parts[0], lineno)?;
    let byte_expr = parse_expr_str(parts[1], lineno)?;
    let cnt = eval(&cnt_expr).map_err(|e| AsmError {
        line: lineno,
        msg: format!(".fill count: {}", e),
    })? as usize;
    let byte = eval(&byte_expr).map_err(|e| AsmError {
        line: lineno,
        msg: format!(".fill byte: {}", e),
    })? as u8;
    Ok((cnt, byte))
}

fn parse_string(s: &str, lineno: usize) -> Result<Vec<u8>, AsmError> {
    if !s.starts_with('"') || !s.ends_with('"') || s.len() < 2 {
        return Err(AsmError {
            line: lineno,
            msg: format!("expected string literal, got '{}'", s),
        });
    }
    let inner = &s[1..s.len() - 1];
    let mut out = Vec::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push(b'\n'),
                Some('r') => out.push(b'\r'),
                Some('t') => out.push(b'\t'),
                Some('\\') => out.push(b'\\'),
                Some('"') => out.push(b'"'),
                Some(c) => {
                    return Err(AsmError {
                        line: lineno,
                        msg: format!("unknown escape '\\{}'", c),
                    });
                }
                None => {
                    return Err(AsmError {
                        line: lineno,
                        msg: "unterminated escape sequence".into(),
                    });
                }
            }
        } else {
            out.push(c as u8);
        }
    }
    Ok(out)
}

fn split_mnemonic(s: &str) -> (&str, &str) {
    let s = s.trim();
    if let Some(i) = s.find([' ', '\t']) {
        (s[..i].trim(), s[i..].trim())
    } else {
        (s, "")
    }
}

// ─── instruction size table ───────────────────────────────────────────────────

fn instr_size_for(mne: &str, full: &str, a_w: Width, i_w: Width, asm: &Assembler) -> Option<usize> {
    let (_mne, ops) = split_mnemonic(full);
    let ops = ops.trim();
    let mode = classify_mode(ops);
    Some(instruction_encoded_size(mne, &mode, a_w, i_w, asm))
}

/// Classify operand string into an addressing mode (for size/encode).
fn classify_mode(ops: &str) -> String {
    ops.to_string()
}

fn instruction_encoded_size(
    mne: &str,
    ops: &str,
    a_w: Width,
    i_w: Width,
    asm: &Assembler,
) -> usize {
    let ops = ops.trim();

    // Resolve an operand expression against the symbols known so far.
    // Pass 1 sees all `NAME = value` constants (hardware registers, WRAM
    // addresses) but not forward code labels; forward labels are only ever
    // jump/branch/data targets, never direct-page operands, so the
    // unresolved fallback below sizes them as 16-bit absolute. Pass 2 of
    // emission makes the SAME value-based choice, and the label-address
    // assertion in `assemble_two_pass` catches any residual disagreement.
    let resolve = |e: &str| -> Option<i64> {
        parse_expr_str(e.trim(), 0)
            .ok()
            .and_then(|x| asm.eval_expr(&x).ok())
    };

    // Implied / accumulator
    match mne {
        "NOP" | "SEI" | "CLI" | "CLC" | "SEC" | "SED" | "CLD" | "CLV" | "TAX" | "TAY" | "TXA"
        | "TYA" | "TSX" | "TXS" | "TXY" | "TYX" | "TCD" | "TDC" | "TCS" | "TSC" | "XCE" | "PHP"
        | "PLP" | "PHA" | "PLA" | "PHX" | "PLX" | "PHY" | "PLY" | "PHB" | "PLB" | "PHD" | "PLD"
        | "PHK" | "RTI" | "RTS" | "RTL" | "WAI" | "STP" | "WDM" | "DEX" | "INX" | "DEY" | "INY"
        | "DEA" | "INA" | "ASL" | "LSR" | "ROL" | "ROR" | "INC" | "DEC"
            if ops.is_empty() || ops == "A" =>
        {
            return 1
        }
        "TAX" | "TAY" | "TXA" | "TYA" | "TSX" | "TXS" | "TXY" | "TYX" | "TCD" | "TDC" | "TCS"
        | "TSC" | "XCE" | "NOP" | "SEI" | "CLI" | "CLC" | "SEC" | "SED" | "CLD" | "CLV" | "PHP"
        | "PLP" | "PHA" | "PLA" | "PHX" | "PLX" | "PHY" | "PLY" | "PHB" | "PLB" | "PHD" | "PLD"
        | "PHK" | "RTI" | "RTS" | "RTL" | "WAI" | "STP" | "WDM" | "DEX" | "INX" | "DEY" | "INY"
        | "DEA" | "INA" => return 1,
        _ => {}
    }

    // MVN / MVP: 3 bytes (opcode + dst + src)
    if mne == "MVN" || mne == "MVP" {
        return 3;
    }

    // Branches: BRA, BEQ, BNE, BCS, BCC, BMI, BPL, BVC, BVS: 2 bytes
    match mne {
        "BRA" | "BEQ" | "BNE" | "BCS" | "BCC" | "BMI" | "BPL" | "BVC" | "BVS" => {
            return 2;
        }
        "BRL" => return 3,
        _ => {}
    }

    // Based on operand form:
    if ops.starts_with('#') {
        // immediate: size depends on register width
        match mne {
            "LDA" | "STA" | "ADC" | "SBC" | "AND" | "ORA" | "EOR" | "CMP" | "BIT" | "LDX"
            | "LDY" | "CPX" | "CPY" => {
                let imm_size = match mne {
                    "LDX" | "CPX" | "STX" => {
                        if i_w == Width::W16 {
                            2
                        } else {
                            1
                        }
                    }
                    "LDY" | "CPY" | "STY" => {
                        if i_w == Width::W16 {
                            2
                        } else {
                            1
                        }
                    }
                    _ => {
                        if a_w == Width::W16 {
                            2
                        } else {
                            1
                        }
                    }
                };
                return 1 + imm_size;
            }
            "REP" | "SEP" | "PEA" | "PEI" | "PER" | "WDM" => return 2,
            _ => return 2, // default: 1 byte imm
        }
    }

    // Long: addr,X 24-bit, or [long]
    if ops.starts_with('[') && ops.ends_with(']') {
        // [dp] or [dp],Y
        return 2;
    }
    if ops.ends_with(']') && ops.contains("],[") {
        return 2; // complex
    }
    if ops.ends_with("],Y") || ops.ends_with("],y") {
        return 2;
    }

    // (indirect): JMP (abs)/JSR (abs,X) carry a 16-bit pointer; every other
    // mnemonic only has the (dp) forms (the encoder emits 1 operand byte).
    if ops.starts_with('(') {
        return if mne == "JMP" || mne == "JSR" { 3 } else { 2 };
    }

    // addr,X: long (4) / dp (2) / abs (3), matching the encoder exactly.
    if ops.ends_with(",X") || ops.ends_with(",x") {
        let base = ops.trim_end_matches(",X").trim_end_matches(",x").trim();
        if is_24bit_addr(base) {
            return 4;
        }
        return match resolve(base) {
            Some(v) if v > 0xFFFF => 4,
            Some(v) if v <= 0xFF => 2,
            _ => 3,
        };
    }
    // addr,Y: only LDX has a dp,Y encoding; the LDA-class encoder always
    // emits abs,Y (3 bytes) regardless of operand value.
    if ops.ends_with(",Y") || ops.ends_with(",y") {
        let base = ops.trim_end_matches(",Y").trim_end_matches(",y").trim();
        if mne == "LDX" {
            return match resolve(base) {
                Some(v) if v <= 0xFF => 2,
                _ => 3,
            };
        }
        return 3;
    }
    if ops.ends_with(",S") || ops.ends_with(",s") {
        return 2;
    }

    // Plain address: value-based dp/abs/long decision identical to the
    // encoder's; unresolved (forward label) operands size as absolute.
    if !ops.is_empty() {
        if mne == "JMP" || mne == "JSR" {
            return 3;
        }
        if mne == "JSL" || mne == "JML" {
            return 4;
        }
        if is_24bit_addr(ops) {
            return 4;
        }
        return match resolve(ops) {
            Some(v) if v > 0xFFFF => 4,
            Some(v) if v <= 0xFF => 2,
            _ => 3,
        };
    }

    1 // implied fallback
}

fn is_24bit_addr(s: &str) -> bool {
    let s = s.trim();
    if s.starts_with('$') {
        return s[1..].len() > 4;
    }
    false
}

fn is_dp_literal(s: &str) -> bool {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('$') {
        if hex.len() <= 2 {
            return true;
        }
        if let Ok(v) = i64::from_str_radix(hex, 16) {
            return v <= 0xFF;
        }
        return false;
    }
    if let Ok(v) = s.parse::<i64>() {
        return v <= 0xFF;
    }
    false
}

// ─── instruction encoder ──────────────────────────────────────────────────────

fn encode_instruction(
    mne: &str,
    ops: &str,
    pc: u32,
    a_w: Width,
    i_w: Width,
    lineno: usize,
    asm: &mut Assembler,
) -> Result<Vec<u8>, AsmError> {
    let ops = ops.trim();

    // Helper: evaluate expression
    let eval = |s: &str| -> Result<i64, AsmError> {
        let expr = asm.parse_expr(s, lineno)?;
        asm.eval_expr(&expr).map_err(|e| AsmError {
            line: lineno,
            msg: e,
        })
    };

    // Helper: emit immediate (1 or 2 bytes depending on width)
    let imm_bytes_a = |val: i64| -> Vec<u8> {
        if a_w == Width::W16 {
            vec![val as u8, (val >> 8) as u8]
        } else {
            vec![val as u8]
        }
    };
    let imm_bytes_i = |val: i64| -> Vec<u8> {
        if i_w == Width::W16 {
            vec![val as u8, (val >> 8) as u8]
        } else {
            vec![val as u8]
        }
    };
    let w16 = |val: i64| -> Vec<u8> { vec![val as u8, (val >> 8) as u8] };
    let w24 = |val: i64| -> Vec<u8> { vec![val as u8, (val >> 8) as u8, (val >> 16) as u8] };

    // Relative branch helper
    let rel8 = |target: i64, pc_after: u32| -> Result<u8, AsmError> {
        let offset = target - pc_after as i64;
        if !(-128..=127).contains(&offset) {
            Err(AsmError {
                line: lineno,
                msg: format!("branch out of range: offset {}", offset),
            })
        } else {
            Ok(offset as i8 as u8)
        }
    };

    // Detect implied/accumulator (ops empty or "A")
    let implied = ops.is_empty() || ops == "A" || ops == "a";

    macro_rules! impl_branch {
        ($opcode:expr) => {{
            let target = eval(ops)?;
            let offset = rel8(target, pc + 2)?;
            return Ok(vec![$opcode, offset]);
        }};
    }

    macro_rules! impl_implied {
        ($opcode:expr) => {{
            return Ok(vec![$opcode]);
        }};
    }

    match mne {
        // ── Transfer / stack ──────────────────────────────────────────────────
        "NOP" => impl_implied!(0xEA),
        "SEI" => impl_implied!(0x78),
        "CLI" => impl_implied!(0x58),
        "CLC" => impl_implied!(0x18),
        "SEC" => impl_implied!(0x38),
        "SED" => impl_implied!(0xF8),
        "CLD" => impl_implied!(0xD8),
        "CLV" => impl_implied!(0xB8),
        "TAX" => impl_implied!(0xAA),
        "TAY" => impl_implied!(0xA8),
        "TXA" => impl_implied!(0x8A),
        "TYA" => impl_implied!(0x98),
        "TSX" => impl_implied!(0xBA),
        "TXS" => impl_implied!(0x9A),
        "TXY" => impl_implied!(0x9B),
        "TYX" => impl_implied!(0xBB),
        "TCD" => impl_implied!(0x5B),
        "TDC" => impl_implied!(0x7B),
        "TCS" => impl_implied!(0x1B),
        "TSC" => impl_implied!(0x3B),
        "XCE" => impl_implied!(0xFB),
        "PHP" => impl_implied!(0x08),
        "PLP" => impl_implied!(0x28),
        "PHA" => impl_implied!(0x48),
        "PLA" => impl_implied!(0x68),
        "PHX" => impl_implied!(0xDA),
        "PLX" => impl_implied!(0xFA),
        "PHY" => impl_implied!(0x5A),
        "PLY" => impl_implied!(0x7A),
        "PHB" => impl_implied!(0x8B),
        "PLB" => impl_implied!(0xAB),
        "PHD" => impl_implied!(0x0B),
        "PLD" => impl_implied!(0x2B),
        "PHK" => impl_implied!(0x4B),
        "RTI" => impl_implied!(0x40),
        "RTS" => impl_implied!(0x60),
        "RTL" => impl_implied!(0x6B),
        "WAI" => impl_implied!(0xCB),
        "STP" => impl_implied!(0xDB),
        "DEX" => impl_implied!(0xCA),
        "INX" => impl_implied!(0xE8),
        "DEY" => impl_implied!(0x88),
        "INY" => impl_implied!(0xC8),
        "DEA" => impl_implied!(0x3A),
        "INA" => impl_implied!(0x1A),

        "ASL" if implied => impl_implied!(0x0A),
        "LSR" if implied => impl_implied!(0x4A),
        "ROL" if implied => impl_implied!(0x2A),
        "ROR" if implied => impl_implied!(0x6A),

        // ── Branches ──────────────────────────────────────────────────────────
        "BRA" => impl_branch!(0x80),
        "BEQ" => impl_branch!(0xF0),
        "BNE" => impl_branch!(0xD0),
        "BCS" => impl_branch!(0xB0),
        "BCC" => impl_branch!(0x90),
        "BMI" => impl_branch!(0x30),
        "BPL" => impl_branch!(0x10),
        "BVC" => impl_branch!(0x50),
        "BVS" => impl_branch!(0x70),
        "BRL" => {
            let target = eval(ops)?;
            let offset = target - (pc as i64 + 3);
            if !(-32768..=32767).contains(&offset) {
                return Err(AsmError {
                    line: lineno,
                    msg: format!("BRL out of range: {}", offset),
                });
            }
            let off = offset as i16 as u16;
            Ok(vec![0x82, off as u8, (off >> 8) as u8])
        }

        // ── REP / SEP ─────────────────────────────────────────────────────────
        "REP" => {
            let v = eval(ops.trim_start_matches('#'))?;
            Ok(vec![0xC2, v as u8])
        }
        "SEP" => {
            let v = eval(ops.trim_start_matches('#'))?;
            Ok(vec![0xE2, v as u8])
        }

        // ── MVN / MVP ─────────────────────────────────────────────────────────
        "MVN" | "MVP" => {
            // MVN #dst,#src — opcode: MVN=$54, MVP=$44
            // encoding: opcode, src_bank, dst_bank (note: src comes second in bytes)
            let parts = parse_list(ops);
            if parts.len() != 2 {
                return Err(AsmError {
                    line: lineno,
                    msg: format!("{} requires two bank operands", mne),
                });
            }
            let dst = eval(parts[0].trim_start_matches('#'))? as u8;
            let src = eval(parts[1].trim_start_matches('#'))? as u8;
            let opcode = if mne == "MVN" { 0x54u8 } else { 0x44u8 };
            Ok(vec![opcode, dst, src])
        }

        // ── LDA ───────────────────────────────────────────────────────────────
        "LDA" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0xA9u8];
                r.extend(imm_bytes_a(v));
                return Ok(r);
            }
            encode_load_store_generic(mne, ops, pc, a_w, i_w, lineno, asm)
        }

        // ── STA ───────────────────────────────────────────────────────────────
        "STA" => encode_load_store_generic(mne, ops, pc, a_w, i_w, lineno, asm),

        // ── STZ ───────────────────────────────────────────────────────────────
        "STZ" => {
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0x64, v as u8]);
            }
            if ops.ends_with(",X") || ops.ends_with(",x") {
                let base = ops.trim_end_matches(",X").trim_end_matches(",x").trim();
                let bv = eval(base)?;
                if bv <= 0xFF {
                    return Ok(vec![0x74, bv as u8]);
                }
                let mut r = vec![0x9Eu8];
                r.extend(w16(bv));
                return Ok(r);
            }
            let mut r = vec![0x9Cu8];
            r.extend(w16(v));
            Ok(r)
        }

        // ── LDX / LDY ─────────────────────────────────────────────────────────
        "LDX" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0xA2u8];
                r.extend(imm_bytes_i(v));
                return Ok(r);
            }
            if ops.ends_with(",Y") || ops.ends_with(",y") {
                let base = ops.trim_end_matches(",Y").trim_end_matches(",y").trim();
                let v = eval(base)?;
                if v <= 0xFF {
                    return Ok(vec![0xB6, v as u8]);
                }
                let mut r = vec![0xBEu8];
                r.extend(w16(v));
                return Ok(r);
            }
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0xA6, v as u8]);
            }
            let mut r = vec![0xAEu8];
            r.extend(w16(v));
            Ok(r)
        }
        "LDY" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0xA0u8];
                r.extend(imm_bytes_i(v));
                return Ok(r);
            }
            if ops.ends_with(",X") || ops.ends_with(",x") {
                let base = ops.trim_end_matches(",X").trim_end_matches(",x").trim();
                let v = eval(base)?;
                if v <= 0xFF {
                    return Ok(vec![0xB4, v as u8]);
                }
                let mut r = vec![0xBCu8];
                r.extend(w16(v));
                return Ok(r);
            }
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0xA4, v as u8]);
            }
            let mut r = vec![0xACu8];
            r.extend(w16(v));
            Ok(r)
        }

        // ── STX / STY ─────────────────────────────────────────────────────────
        "STX" => {
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0x86, v as u8]);
            }
            let mut r = vec![0x8Eu8];
            r.extend(w16(v));
            Ok(r)
        }
        "STY" => {
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0x84, v as u8]);
            }
            let mut r = vec![0x8Cu8];
            r.extend(w16(v));
            Ok(r)
        }

        // ── ADC / SBC ─────────────────────────────────────────────────────────
        "ADC" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0x69u8];
                r.extend(imm_bytes_a(v));
                return Ok(r);
            }
            encode_arith_generic(mne, ops, pc, a_w, i_w, lineno, asm)
        }
        "SBC" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0xE9u8];
                r.extend(imm_bytes_a(v));
                return Ok(r);
            }
            encode_arith_generic(mne, ops, pc, a_w, i_w, lineno, asm)
        }

        // ── AND / ORA / EOR ───────────────────────────────────────────────────
        "AND" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0x29u8];
                r.extend(imm_bytes_a(v));
                return Ok(r);
            }
            encode_arith_generic(mne, ops, pc, a_w, i_w, lineno, asm)
        }
        "ORA" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0x09u8];
                r.extend(imm_bytes_a(v));
                return Ok(r);
            }
            encode_arith_generic(mne, ops, pc, a_w, i_w, lineno, asm)
        }
        "EOR" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0x49u8];
                r.extend(imm_bytes_a(v));
                return Ok(r);
            }
            encode_arith_generic(mne, ops, pc, a_w, i_w, lineno, asm)
        }

        // ── CMP / CPX / CPY ───────────────────────────────────────────────────
        "CMP" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0xC9u8];
                r.extend(imm_bytes_a(v));
                return Ok(r);
            }
            encode_arith_generic(mne, ops, pc, a_w, i_w, lineno, asm)
        }
        "CPX" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0xE0u8];
                r.extend(imm_bytes_i(v));
                return Ok(r);
            }
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0xE4, v as u8]);
            }
            let mut r = vec![0xECu8];
            r.extend(w16(v));
            Ok(r)
        }
        "CPY" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0xC0u8];
                r.extend(imm_bytes_i(v));
                return Ok(r);
            }
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0xC4, v as u8]);
            }
            let mut r = vec![0xCCu8];
            r.extend(w16(v));
            Ok(r)
        }

        // ── INC / DEC ─────────────────────────────────────────────────────────
        "INC" if !implied => {
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0xE6, v as u8]);
            }
            let mut r = vec![0xEEu8];
            r.extend(w16(v));
            Ok(r)
        }
        "DEC" if !implied => {
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0xC6, v as u8]);
            }
            let mut r = vec![0xCEu8];
            r.extend(w16(v));
            Ok(r)
        }
        "INC" if implied => impl_implied!(0x1A),
        "DEC" if implied => impl_implied!(0x3A),

        // ── ASL / LSR / ROL / ROR (memory) ───────────────────────────────────
        "ASL" => {
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0x06, v as u8]);
            }
            let mut r = vec![0x0Eu8];
            r.extend(w16(v));
            Ok(r)
        }
        "LSR" => {
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0x46, v as u8]);
            }
            let mut r = vec![0x4Eu8];
            r.extend(w16(v));
            Ok(r)
        }
        "ROL" => {
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0x26, v as u8]);
            }
            let mut r = vec![0x2Eu8];
            r.extend(w16(v));
            Ok(r)
        }
        "ROR" => {
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0x66, v as u8]);
            }
            let mut r = vec![0x6Eu8];
            r.extend(w16(v));
            Ok(r)
        }

        // ── BIT ──────────────────────────────────────────────────────────────
        "BIT" => {
            if ops.starts_with('#') {
                let v = eval(&ops[1..])?;
                let mut r = vec![0x89u8];
                r.extend(imm_bytes_a(v));
                return Ok(r);
            }
            let v = eval(ops)?;
            if v <= 0xFF {
                return Ok(vec![0x24, v as u8]);
            }
            let mut r = vec![0x2Cu8];
            r.extend(w16(v));
            Ok(r)
        }

        // ── JSR / JMP / JSL / JML ────────────────────────────────────────────
        "JSR" => {
            if ops.starts_with('(') {
                // JSR (abs,X)
                let inner = ops.trim_start_matches('(').trim_end_matches(')');
                let inner = inner.trim_end_matches(",X").trim_end_matches(",x");
                let v = eval(inner.trim())?;
                let mut r = vec![0xFCu8];
                r.extend(w16(v));
                return Ok(r);
            }
            let v = eval(ops)?;
            let mut r = vec![0x20u8];
            r.extend(w16(v));
            Ok(r)
        }
        "JSL" => {
            let v = eval(ops)?;
            let mut r = vec![0x22u8];
            r.extend(w24(v));
            Ok(r)
        }
        "JMP" => {
            if ops.starts_with('(') && !ops.contains(',') {
                let inner = ops.trim_start_matches('(').trim_end_matches(')');
                let v = eval(inner)?;
                let mut r = vec![0x6Cu8];
                r.extend(w16(v));
                return Ok(r);
            }
            if ops.starts_with('(') && (ops.contains(",X)") || ops.contains(",x)")) {
                let inner = ops
                    .trim_start_matches('(')
                    .trim_end_matches(')')
                    .trim_end_matches(",X")
                    .trim_end_matches(",x");
                let v = eval(inner.trim())?;
                let mut r = vec![0x7Cu8];
                r.extend(w16(v));
                return Ok(r);
            }
            if ops.starts_with('[') {
                let inner = ops.trim_start_matches('[').trim_end_matches(']');
                let v = eval(inner)?;
                let mut r = vec![0xDCu8];
                r.extend(w16(v));
                return Ok(r);
            }
            let v = eval(ops)?;
            if is_24bit_addr(ops) || v > 0xFFFF {
                let mut r = vec![0x5Cu8];
                r.extend(w24(v));
                return Ok(r);
            }
            let mut r = vec![0x4Cu8];
            r.extend(w16(v));
            Ok(r)
        }
        "JML" => {
            let v = eval(ops)?;
            let mut r = vec![0x5Cu8];
            r.extend(w24(v));
            Ok(r)
        }

        // ── PUSH immediate ───────────────────────────────────────────────────
        "PEA" => {
            let v = eval(ops.trim_start_matches('#'))?;
            let mut r = vec![0xF4u8];
            r.extend(w16(v));
            Ok(r)
        }

        _ => {
            // For any other mnemonic, try the generic encoder
            Err(AsmError {
                line: lineno,
                msg: format!("unknown mnemonic '{}'", mne),
            })
        }
    }
}

// ─── generic load/store encoder ──────────────────────────────────────────────

fn encode_load_store_generic(
    mne: &str,
    ops: &str,
    _pc: u32,
    _a_w: Width,
    _i_w: Width,
    lineno: usize,
    asm: &mut Assembler,
) -> Result<Vec<u8>, AsmError> {
    let eval = |s: &str| -> Result<i64, AsmError> {
        let expr = asm.parse_expr(s, lineno)?;
        asm.eval_expr(&expr).map_err(|e| AsmError {
            line: lineno,
            msg: e,
        })
    };
    let w16 = |val: i64| -> Vec<u8> { vec![val as u8, (val >> 8) as u8] };
    let w24 = |val: i64| -> Vec<u8> { vec![val as u8, (val >> 8) as u8, (val >> 16) as u8] };

    // Indirect modes
    if ops.starts_with('(') {
        if ops.contains(",X)") || ops.contains(",x)") {
            // (dp,X)
            let inner = ops.trim_start_matches('(');
            let inner = inner
                .trim_end_matches(')')
                .trim_end_matches(",X")
                .trim_end_matches(",x");
            let v = eval(inner.trim())?;
            let opcode = match mne {
                "LDA" => 0xA1u8,
                "STA" => 0x81u8,
                "ADC" => 0x61u8,
                "SBC" => 0xE1u8,
                "AND" => 0x21u8,
                "ORA" => 0x01u8,
                "EOR" => 0x41u8,
                "CMP" => 0xC1u8,
                _ => {
                    return Err(AsmError {
                        line: lineno,
                        msg: format!("{} (dp,X) not supported", mne),
                    })
                }
            };
            return Ok(vec![opcode, v as u8]);
        }
        if ops.ends_with("),Y") || ops.ends_with("),y") {
            let inner = ops
                .trim_start_matches('(')
                .trim_end_matches("),Y")
                .trim_end_matches("),y");
            let v = eval(inner.trim())?;
            let opcode = match mne {
                "LDA" => 0xB1u8,
                "STA" => 0x91u8,
                "ADC" => 0x71u8,
                "SBC" => 0xF1u8,
                "AND" => 0x31u8,
                "ORA" => 0x11u8,
                "EOR" => 0x51u8,
                "CMP" => 0xD1u8,
                _ => {
                    return Err(AsmError {
                        line: lineno,
                        msg: format!("{} (dp),Y not supported", mne),
                    })
                }
            };
            return Ok(vec![opcode, v as u8]);
        }
        // (dp) — indirect
        let inner = ops.trim_start_matches('(').trim_end_matches(')');
        let v = eval(inner.trim())?;
        let opcode = match mne {
            "LDA" => 0xB2u8,
            "STA" => 0x92u8,
            "ADC" => 0x72u8,
            "SBC" => 0xF2u8,
            "AND" => 0x32u8,
            "ORA" => 0x12u8,
            "EOR" => 0x52u8,
            "CMP" => 0xD2u8,
            _ => {
                return Err(AsmError {
                    line: lineno,
                    msg: format!("{} (dp) not supported", mne),
                })
            }
        };
        return Ok(vec![opcode, v as u8]);
    }

    if ops.starts_with('[') {
        if ops.ends_with("],Y") || ops.ends_with("],y") {
            let inner = ops
                .trim_start_matches('[')
                .trim_end_matches("],Y")
                .trim_end_matches("],y");
            let v = eval(inner.trim())?;
            let opcode = match mne {
                "LDA" => 0xB7u8,
                "STA" => 0x97u8,
                "ADC" => 0x77u8,
                "SBC" => 0xF7u8,
                "AND" => 0x37u8,
                "ORA" => 0x17u8,
                "EOR" => 0x57u8,
                "CMP" => 0xD7u8,
                _ => {
                    return Err(AsmError {
                        line: lineno,
                        msg: format!("{} [dp],Y not supported", mne),
                    })
                }
            };
            return Ok(vec![opcode, v as u8]);
        }
        let inner = ops.trim_start_matches('[').trim_end_matches(']');
        let v = eval(inner.trim())?;
        let opcode = match mne {
            "LDA" => 0xA7u8,
            "STA" => 0x87u8,
            "ADC" => 0x67u8,
            "SBC" => 0xE7u8,
            "AND" => 0x27u8,
            "ORA" => 0x07u8,
            "EOR" => 0x47u8,
            "CMP" => 0xC7u8,
            _ => {
                return Err(AsmError {
                    line: lineno,
                    msg: format!("{} [dp] not supported", mne),
                })
            }
        };
        return Ok(vec![opcode, v as u8]);
    }

    // addr,X / addr,Y
    if ops.ends_with(",X") || ops.ends_with(",x") {
        let base = ops.trim_end_matches(",X").trim_end_matches(",x").trim();
        let v = eval(base)?;
        if is_24bit_addr(base) || v > 0xFFFF {
            // long,X
            let opcode = match mne {
                "LDA" => 0xBFu8,
                "STA" => 0x9Fu8,
                "ADC" => 0x7Fu8,
                "SBC" => 0xFFu8,
                "AND" => 0x3Fu8,
                "ORA" => 0x1Fu8,
                "EOR" => 0x5Fu8,
                "CMP" => 0xDFu8,
                _ => {
                    return Err(AsmError {
                        line: lineno,
                        msg: format!("{} long,X not supported", mne),
                    })
                }
            };
            let mut r = vec![opcode];
            r.extend(vec![v as u8, (v >> 8) as u8, (v >> 16) as u8]);
            return Ok(r);
        }
        if v <= 0xFF {
            let opcode = match mne {
                "LDA" => 0xB5u8,
                "STA" => 0x95u8,
                "ADC" => 0x75u8,
                "SBC" => 0xF5u8,
                "AND" => 0x35u8,
                "ORA" => 0x15u8,
                "EOR" => 0x55u8,
                "CMP" => 0xD5u8,
                _ => {
                    return Err(AsmError {
                        line: lineno,
                        msg: format!("{} dp,X not supported", mne),
                    })
                }
            };
            return Ok(vec![opcode, v as u8]);
        }
        let opcode = match mne {
            "LDA" => 0xBDu8,
            "STA" => 0x9Du8,
            "ADC" => 0x7Du8,
            "SBC" => 0xFDu8,
            "AND" => 0x3Du8,
            "ORA" => 0x1Du8,
            "EOR" => 0x5Du8,
            "CMP" => 0xDDu8,
            _ => {
                return Err(AsmError {
                    line: lineno,
                    msg: format!("{} abs,X not supported", mne),
                })
            }
        };
        let mut r = vec![opcode];
        r.extend(w16(v));
        return Ok(r);
    }
    if ops.ends_with(",Y") || ops.ends_with(",y") {
        let base = ops.trim_end_matches(",Y").trim_end_matches(",y").trim();
        let v = eval(base)?;
        if v <= 0xFF {
            // dp,Y only for LDX
            let opcode = match mne {
                "LDA" => 0xB9u8,
                "STA" => 0x99u8,
                "ADC" => 0x79u8,
                "SBC" => 0xF9u8,
                "AND" => 0x39u8,
                "ORA" => 0x19u8,
                "EOR" => 0x59u8,
                "CMP" => 0xD9u8,
                _ => {
                    return Err(AsmError {
                        line: lineno,
                        msg: format!("{} dp,Y not supported", mne),
                    })
                }
            };
            // For small addrs still use abs,Y encoding (dp,Y not available for LDA etc)
            let mut r = vec![opcode];
            r.extend(w16(v));
            return Ok(r);
        }
        let opcode = match mne {
            "LDA" => 0xB9u8,
            "STA" => 0x99u8,
            "ADC" => 0x79u8,
            "SBC" => 0xF9u8,
            "AND" => 0x39u8,
            "ORA" => 0x19u8,
            "EOR" => 0x59u8,
            "CMP" => 0xD9u8,
            _ => {
                return Err(AsmError {
                    line: lineno,
                    msg: format!("{} abs,Y not supported", mne),
                })
            }
        };
        let mut r = vec![opcode];
        r.extend(w16(v));
        return Ok(r);
    }

    // stack relative: dp,S
    if ops.ends_with(",S") || ops.ends_with(",s") {
        let base = ops.trim_end_matches(",S").trim_end_matches(",s").trim();
        let v = eval(base)?;
        let opcode = match mne {
            "LDA" => 0xA3u8,
            "STA" => 0x83u8,
            "ADC" => 0x63u8,
            "SBC" => 0xE3u8,
            "AND" => 0x23u8,
            "ORA" => 0x03u8,
            "EOR" => 0x43u8,
            "CMP" => 0xC3u8,
            _ => {
                return Err(AsmError {
                    line: lineno,
                    msg: format!("{} sr,S not supported", mne),
                })
            }
        };
        return Ok(vec![opcode, v as u8]);
    }

    // Direct / absolute
    let v = eval(ops)?;
    if is_24bit_addr(ops) || v > 0xFFFF {
        // long
        let opcode = match mne {
            "LDA" => 0xAFu8,
            "STA" => 0x8Fu8,
            "ADC" => 0x6Fu8,
            "SBC" => 0xEFu8,
            "AND" => 0x2Fu8,
            "ORA" => 0x0Fu8,
            "EOR" => 0x4Fu8,
            "CMP" => 0xCFu8,
            _ => {
                return Err(AsmError {
                    line: lineno,
                    msg: format!("{} long not supported", mne),
                })
            }
        };
        let mut r = vec![opcode];
        r.extend(w24(v));
        return Ok(r);
    }
    if is_dp_literal(ops) || v <= 0xFF {
        let opcode = match mne {
            "LDA" => 0xA5u8,
            "STA" => 0x85u8,
            "ADC" => 0x65u8,
            "SBC" => 0xE5u8,
            "AND" => 0x25u8,
            "ORA" => 0x05u8,
            "EOR" => 0x45u8,
            "CMP" => 0xC5u8,
            _ => {
                return Err(AsmError {
                    line: lineno,
                    msg: format!("{} dp not supported", mne),
                })
            }
        };
        return Ok(vec![opcode, v as u8]);
    }
    // absolute
    let opcode = match mne {
        "LDA" => 0xADu8,
        "STA" => 0x8Du8,
        "ADC" => 0x6Du8,
        "SBC" => 0xEDu8,
        "AND" => 0x2Du8,
        "ORA" => 0x0Du8,
        "EOR" => 0x4Du8,
        "CMP" => 0xCDu8,
        _ => {
            return Err(AsmError {
                line: lineno,
                msg: format!("{} abs not supported", mne),
            })
        }
    };
    let mut r = vec![opcode];
    r.extend(w16(v));
    Ok(r)
}

fn encode_arith_generic(
    mne: &str,
    ops: &str,
    pc: u32,
    a_w: Width,
    i_w: Width,
    lineno: usize,
    asm: &mut Assembler,
) -> Result<Vec<u8>, AsmError> {
    encode_load_store_generic(mne, ops, pc, a_w, i_w, lineno, asm)
}

// ─── public entry point ───────────────────────────────────────────────────────

/// Assemble `src` and return (bytes, base_origin). Deterministic.
pub fn assemble(src: &str) -> Result<(Vec<u8>, u32), AsmError> {
    let mut asm = Assembler::new();
    asm.assemble_two_pass(src)
}

// ─── unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Assemble a single line at org $8000, return emitted bytes.
    fn asm_one(line: &str) -> Vec<u8> {
        let src = format!(".org $8000\n{}\n", line);
        let (bytes, _base) = assemble(&src).unwrap_or_else(|_| panic!("asm failed: {}", line));
        bytes
    }

    /// Assemble with a_width state.
    fn asm_one_a16(line: &str) -> Vec<u8> {
        let src = format!(".org $8000\n.a16\n{}\n", line);
        let (bytes, _base) = assemble(&src).unwrap_or_else(|_| panic!("asm_a16 failed: {}", line));
        bytes
    }

    /// Assemble with i_width state.
    fn asm_one_i16(line: &str) -> Vec<u8> {
        let src = format!(".org $8000\n.i16\n{}\n", line);
        let (bytes, _base) = assemble(&src).unwrap_or_else(|_| panic!("asm_i16 failed: {}", line));
        bytes
    }

    // ── immediate mode ────────────────────────────────────────────────────────

    #[test]
    fn lda_imm_8bit() {
        // LDA #$42 in 8-bit mode: A9 42
        assert_eq!(asm_one("LDA #$42"), vec![0xA9, 0x42]);
    }

    #[test]
    fn lda_imm_16bit() {
        // LDA #$1234 in 16-bit mode: A9 34 12
        assert_eq!(asm_one_a16("LDA #$1234"), vec![0xA9, 0x34, 0x12]);
    }

    #[test]
    fn ldx_imm_16bit() {
        // LDX #$1FFF in 16-bit index mode: A2 FF 1F
        assert_eq!(asm_one_i16("LDX #$1FFF"), vec![0xA2, 0xFF, 0x1F]);
    }

    #[test]
    fn lda_dp() {
        // LDA $42 (direct page): A5 42
        assert_eq!(asm_one("LDA $42"), vec![0xA5, 0x42]);
    }

    #[test]
    fn lda_dp_x() {
        // LDA $12,X: B5 12
        assert_eq!(asm_one("LDA $12,X"), vec![0xB5, 0x12]);
    }

    #[test]
    fn lda_abs() {
        // LDA $2100 (absolute): AD 00 21
        assert_eq!(asm_one("LDA $2100"), vec![0xAD, 0x00, 0x21]);
    }

    #[test]
    fn lda_abs_x() {
        // LDA $2100,X: BD 00 21
        assert_eq!(asm_one("LDA $2100,X"), vec![0xBD, 0x00, 0x21]);
    }

    #[test]
    fn lda_abs_y() {
        // LDA $4218,Y: B9 18 42
        assert_eq!(asm_one("LDA $4218,Y"), vec![0xB9, 0x18, 0x42]);
    }

    #[test]
    fn sta_long_x() {
        // STA $7E1000,X (long,X): 9F 00 10 7E
        assert_eq!(asm_one("STA $7E1000,X"), vec![0x9F, 0x00, 0x10, 0x7E]);
    }

    #[test]
    fn lda_long() {
        // LDA $7E0000 (long): AF 00 00 7E
        assert_eq!(asm_one("LDA $7E0000"), vec![0xAF, 0x00, 0x00, 0x7E]);
    }

    // ── JSR / JMP ─────────────────────────────────────────────────────────────

    #[test]
    fn jsr_abs() {
        // JSR $8000: 20 00 80
        assert_eq!(asm_one("JSR $8000"), vec![0x20, 0x00, 0x80]);
    }

    #[test]
    fn jmp_abs() {
        // JMP $8000: 4C 00 80
        assert_eq!(asm_one("JMP $8000"), vec![0x4C, 0x00, 0x80]);
    }

    #[test]
    fn jmp_indirect() {
        // JMP ($FFFC): 6C FC FF
        assert_eq!(asm_one("JMP ($FFFC)"), vec![0x6C, 0xFC, 0xFF]);
    }

    // ── MVN / MVP ─────────────────────────────────────────────────────────────

    #[test]
    fn mvn_banks() {
        // MVN #$00,#$00: 54 00 00
        assert_eq!(asm_one("MVN #$00,#$00"), vec![0x54, 0x00, 0x00]);
    }

    #[test]
    fn mvn_bank_7e() {
        // MVN #$7E,#$7E: 54 7E 7E
        assert_eq!(asm_one("MVN #$7E,#$7E"), vec![0x54, 0x7E, 0x7E]);
    }

    // ── Branch instructions ───────────────────────────────────────────────────

    #[test]
    fn bne_forward() {
        // BNE to label 2 bytes ahead: D0 02
        let src = ".org $8000\nBNE skip\nNOP\nNOP\nskip:\nNOP\n";
        let (bytes, _) = assemble(src).expect("bne forward");
        // BNE D0 02, NOP EA, NOP EA, NOP EA
        assert_eq!(&bytes[..2], &[0xD0, 0x02]);
    }

    #[test]
    fn bra_backward() {
        // BRA back to self: 80 FE (-2)
        let src = ".org $8000\ntrap:\nBRA trap\n";
        let (bytes, _) = assemble(src).expect("bra backward");
        assert_eq!(bytes, vec![0x80, 0xFE]);
    }

    // ── REP / SEP ─────────────────────────────────────────────────────────────

    #[test]
    fn rep_30() {
        // REP #$30: C2 30
        assert_eq!(asm_one("REP #$30"), vec![0xC2, 0x30]);
    }

    #[test]
    fn sep_20() {
        // SEP #$20: E2 20
        assert_eq!(asm_one("SEP #$20"), vec![0xE2, 0x20]);
    }

    // ── Implied instructions ──────────────────────────────────────────────────

    #[test]
    fn implied_instrs() {
        assert_eq!(asm_one("NOP"), vec![0xEA]);
        assert_eq!(asm_one("SEI"), vec![0x78]);
        assert_eq!(asm_one("CLC"), vec![0x18]);
        assert_eq!(asm_one("SEC"), vec![0x38]);
        assert_eq!(asm_one("XCE"), vec![0xFB]);
        assert_eq!(asm_one("TCD"), vec![0x5B]);
        assert_eq!(asm_one("TXS"), vec![0x9A]);
        assert_eq!(asm_one("PHA"), vec![0x48]);
        assert_eq!(asm_one("PLB"), vec![0xAB]);
        assert_eq!(asm_one("PHK"), vec![0x4B]);
        assert_eq!(asm_one("RTI"), vec![0x40]);
        assert_eq!(asm_one("RTS"), vec![0x60]);
        assert_eq!(asm_one("WAI"), vec![0xCB]);
        assert_eq!(asm_one("STP"), vec![0xDB]);
        assert_eq!(asm_one("INX"), vec![0xE8]);
        assert_eq!(asm_one("INY"), vec![0xC8]);
        assert_eq!(asm_one("DEX"), vec![0xCA]);
        assert_eq!(asm_one("DEY"), vec![0x88]);
    }

    #[test]
    fn asl_accumulator() {
        assert_eq!(asm_one("ASL A"), vec![0x0A]);
        assert_eq!(asm_one("LSR A"), vec![0x4A]);
        assert_eq!(asm_one("ROL A"), vec![0x2A]);
        assert_eq!(asm_one("ROR A"), vec![0x6A]);
    }

    // ── Arithmetic ───────────────────────────────────────────────────────────

    #[test]
    fn adc_imm_8bit() {
        // ADC #$01: 69 01
        assert_eq!(asm_one("ADC #$01"), vec![0x69, 0x01]);
    }

    #[test]
    fn sbc_imm_16bit() {
        // SBC #$0001 in 16-bit: E9 01 00
        assert_eq!(asm_one_a16("SBC #$0001"), vec![0xE9, 0x01, 0x00]);
    }

    #[test]
    fn and_imm_8bit() {
        // AND #$5A: 29 5A
        assert_eq!(asm_one("AND #$5A"), vec![0x29, 0x5A]);
    }

    #[test]
    fn ora_imm() {
        // ORA #$A5: 09 A5
        assert_eq!(asm_one("ORA #$A5"), vec![0x09, 0xA5]);
    }

    #[test]
    fn eor_imm() {
        // EOR #$FF: 49 FF
        assert_eq!(asm_one("EOR #$FF"), vec![0x49, 0xFF]);
    }

    #[test]
    fn cmp_imm_16bit() {
        // CMP #$1234 in 16-bit: C9 34 12
        assert_eq!(asm_one_a16("CMP #$1234"), vec![0xC9, 0x34, 0x12]);
    }

    // ── Directives ───────────────────────────────────────────────────────────

    #[test]
    fn directive_db() {
        let (bytes, _) = assemble(".org $8000\n.db $FF,$00,$AB\n").unwrap();
        assert_eq!(bytes, vec![0xFF, 0x00, 0xAB]);
    }

    #[test]
    fn directive_dw() {
        let (bytes, _) = assemble(".org $8000\n.dw $1234\n").unwrap();
        assert_eq!(bytes, vec![0x34, 0x12]);
    }

    #[test]
    fn directive_fill() {
        let (bytes, _) = assemble(".org $8000\n.fill 4, $00\n").unwrap();
        assert_eq!(bytes, vec![0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn directive_ascii() {
        let (bytes, _) = assemble(".org $8000\n.ascii \"ABC\"\n").unwrap();
        assert_eq!(bytes, vec![b'A', b'B', b'C']);
    }

    // ── Label resolution ─────────────────────────────────────────────────────

    #[test]
    fn forward_label_jsr() {
        let src = ".org $8000\nJSR target\nNOP\ntarget:\nRTS\n";
        let (bytes, _) = assemble(src).unwrap();
        // JSR is 3 bytes ($8000-$8002), NOP is 1 byte ($8003), target: at $8004
        // JSR target: 20 04 80, NOP: EA, RTS: 60
        assert_eq!(&bytes[0..3], &[0x20, 0x04, 0x80]);
        assert_eq!(bytes[3], 0xEA);
        assert_eq!(bytes[4], 0x60);
    }

    #[test]
    fn backward_label_jmp() {
        let src = ".org $8000\nstart:\nNOP\nJMP start\n";
        let (bytes, _) = assemble(src).unwrap();
        // NOP: EA, JMP $8000: 4C 00 80
        assert_eq!(bytes[0], 0xEA);
        assert_eq!(&bytes[1..4], &[0x4C, 0x00, 0x80]);
    }

    #[test]
    fn constant_assignment() {
        let src = ".org $8000\nMY_CONST = $42\nLDA #MY_CONST\n";
        let (bytes, _) = assemble(src).unwrap();
        assert_eq!(bytes, vec![0xA9, 0x42]);
    }

    // ── Error cases ──────────────────────────────────────────────────────────

    #[test]
    fn unknown_mnemonic_errors() {
        let result = assemble(".org $8000\nFOOBAR $00\n");
        assert!(result.is_err(), "unknown mnemonic should fail");
        let err = result.unwrap_err();
        assert!(err.msg.contains("FOOBAR"), "error should mention mnemonic");
    }

    #[test]
    fn branch_out_of_range_errors() {
        // Generate a source with BEQ to a label > 127 bytes away
        let mut src = ".org $8000\nBEQ far_label\n".to_string();
        for _ in 0..130 {
            src.push_str("NOP\n");
        }
        src.push_str("far_label:\nNOP\n");
        let result = assemble(&src);
        assert!(result.is_err(), "out-of-range branch should fail");
    }

    // ── .org changes ─────────────────────────────────────────────────────────

    #[test]
    fn org_sets_base() {
        let (bytes, base) = assemble(".org $8000\nNOP\n").unwrap();
        assert_eq!(base, 0x8000);
        assert_eq!(bytes, vec![0xEA]);
    }

    #[test]
    fn multiple_orgs() {
        // First org at $8000, second at $8010: bytes from $8000..$8011
        let src = ".org $8000\nNOP\n.org $8010\nRTS\n";
        let (bytes, base) = assemble(src).unwrap();
        assert_eq!(base, 0x8000);
        assert_eq!(bytes[0], 0xEA); // NOP at $8000
        assert_eq!(bytes[0x10], 0x60); // RTS at $8010
                                       // Gap bytes between $8001 and $800F should be zero
        for (i, b) in bytes.iter().enumerate().take(0x10).skip(1) {
            assert_eq!(*b, 0, "gap byte at offset {} should be zero", i);
        }
    }
}
