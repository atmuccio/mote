//! `java/jdbc-zero-index` — JDBC parameter and column indexes start at 1.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["jdbc", "api-misuse"],
    remediation_minutes: 2,
    description: r#"Detects JDBC calls that pass index `0` (or a negative index) where JDBC counts
from 1: `PreparedStatement.setXxx(0, …)`, `CallableStatement.registerOutParameter(0, …)`,
`ResultSet.getXxx(0)` / `updateXxx(0, …)`, and `ResultSetMetaData.getColumnXxx(0)`.

**Why it matters.** Parameter and column positions in JDBC are 1-based. Index 0 does not
exist, so the driver throws `SQLException` ("parameter index out of range", "invalid column
index") every time the statement runs — or, with a shifted loop, binds every value to the
wrong placeholder.

**How to fix.** Start at 1.

```java
// Noncompliant
ps.setString(0, name);
long id = rs.getLong(0);

// Compliant
ps.setString(1, name);
long id = rs.getLong(1);
```

Only receivers the resolver proves to be a `java.sql` statement, result set, or result-set
metadata are reported. Methods whose `int` argument is not an index (`setFetchSize`,
`setMaxRows`, `absolute`, …) are not checked."#,
    ..RuleMeta::new(
        "java/jdbc-zero-index",
        "java",
        "JDBC index 0 (indexes start at 1)",
    )
};

#[derive(Default)]
pub struct JdbcZeroIndex {
    sql: util::Gate,
}

const NAMES: &[&str] = &[
    // PreparedStatement / CallableStatement setters
    "setString",
    "setNString",
    "setInt",
    "setLong",
    "setShort",
    "setByte",
    "setBoolean",
    "setFloat",
    "setDouble",
    "setBigDecimal",
    "setBytes",
    "setDate",
    "setTime",
    "setTimestamp",
    "setObject",
    "setNull",
    "setBlob",
    "setClob",
    "setNClob",
    "setArray",
    "setRef",
    "setURL",
    "setRowId",
    "setSQLXML",
    "setAsciiStream",
    "setBinaryStream",
    "setCharacterStream",
    "setNCharacterStream",
    "registerOutParameter",
    // ResultSet / CallableStatement getters
    "getString",
    "getNString",
    "getInt",
    "getLong",
    "getShort",
    "getByte",
    "getBoolean",
    "getFloat",
    "getDouble",
    "getBigDecimal",
    "getBytes",
    "getDate",
    "getTime",
    "getTimestamp",
    "getObject",
    "getBlob",
    "getClob",
    "getNClob",
    "getArray",
    "getRef",
    "getURL",
    "getRowId",
    "getSQLXML",
    "getAsciiStream",
    "getBinaryStream",
    "getCharacterStream",
    "getNCharacterStream",
    // ResultSet updaters
    "updateString",
    "updateInt",
    "updateLong",
    "updateDouble",
    "updateBigDecimal",
    "updateBoolean",
    "updateDate",
    "updateTimestamp",
    "updateObject",
    "updateNull",
    "updateBytes",
    // ResultSetMetaData
    "getColumnName",
    "getColumnLabel",
    "getColumnType",
    "getColumnTypeName",
    "getColumnClassName",
    "getColumnDisplaySize",
    "getPrecision",
    "getScale",
    "getTableName",
    "getSchemaName",
    "getCatalogName",
    "isNullable",
    "isAutoIncrement",
    "isSigned",
    "isReadOnly",
    "isWritable",
    "isCaseSensitive",
    "isSearchable",
    "isCurrency",
];

const RECEIVERS: &[&str] = &[
    "java.sql.PreparedStatement",
    "java.sql.ResultSet",
    "java.sql.ResultSetMetaData",
];

impl<'t> JavaRule<'t> for JdbcZeroIndex {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        NAMES
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !NAMES.contains(&name) {
            return;
        }
        let Some(first) = util::arg(node, 0) else {
            return;
        };
        let Some(v) = util::int_value(ctx, first) else {
            return;
        };
        if v > 0 || !self.sql.open(ctx, "java.sql") {
            return;
        }
        if !util::receiver_is(ctx, node, RECEIVERS) {
            return;
        }
        ctx.emit(
            first,
            format!(
                "Use index 1 or higher: JDBC indexes start at 1, so `{name}({v}, …)` always fails"
            ),
        );
    }
}
