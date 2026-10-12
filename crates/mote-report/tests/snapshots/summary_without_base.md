<!-- mote:summary -->
## :x: Mote: quality gate failed

No base to compare with: the gate applies to all findings.

| Condition | Result | Actual | Threshold |
|---|---|---:|---:|
| Findings | :x: failed | 1 | ≤ 0 |
| Duplication on new code | :fast_forward: skipped (no base to compare with) |  | ≤ 3.0% |

### Findings

| Severity | Total |
|---|---:|
| HIGH | 1 |
| MEDIUM | 3 |
| LOW | 1 |

### Top findings (5 of 5)

| Severity | Location | Rule | Message |
|---|---|---|---|
| HIGH | `src/a/Order Service.java:3` | `java/self-assignment` | Remove this self-assignment of `total` \| it has no effect |
| MEDIUM | `src/a/Order Service.java:20` | `java/cognitive-complexity` | Reduce the cognitive complexity of `total` from 22 to 15 |
| MEDIUM | `src/a/Order Service.java:40` | `java/duplicated-block` | This block of 21 lines is duplicated in 1 other place |
| MEDIUM | `src/b/Legacy.java:2` | `java/empty-catch-block` | Remove this empty catch block or explain why the exception is ignored |
| LOW | `src/c/Util.java:1` | `java/todo-comment` | Complete the task in this TODO comment |

### Measures

| Measure | Overall |
|---|---:|
| Lines | 300 |
| Duplication | 14.0% |
| Estimated effort | 0.9 h |
| Cognitive complexity | 57 |
| Cyclomatic complexity | 80 |
| Maintainability grade | **D** (3.61) |
| Reliability grade | **E** (41.67) |

_Grades run from A (best) to E: maintainability by the estimated hours of fixes per 1,000 lines of code, reliability by weighted reliability findings per 10,000 lines._

<details><summary>Totals</summary>

| Total | Value |
|---|---:|
| Files | 4 (3 analyzed, 1 skipped) |
| Lines of code | 240 |
| Findings | 5 (1 HIGH, 3 MEDIUM, 1 LOW) |
| Suppressed | 1 |
| Estimated effort | 0.9 h (reliability < 0.1 h, maintainability 0.9 h) |
| Duplicated blocks | 1 |

</details>
