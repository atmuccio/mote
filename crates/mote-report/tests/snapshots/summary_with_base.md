<!-- mote:summary -->
## :x: Mote: quality gate failed

Compared with `origin/main` (merge base `fedcba987654`): **2** files changed, **38** new lines.

| Condition | Result | Actual | Threshold |
|---|---|---:|---:|
| New findings | :x: failed | 1 | ≤ 0 |
| Duplication on new code | :x: failed | 55.3% | ≤ 3.0% |
| Coverage on new code | :white_check_mark: passed | 80.0% | ≥ 80.0% |
| Maintainability grade on new code | :x: failed | E | B or better |

### New findings

| Severity | New | Worsened | Fixed | Total |
|---|---:|---:|---:|---:|
| HIGH | 1 | 0 | 0 | 1 |
| MEDIUM | 1 | 1 | 2 | 3 |
| LOW | 1 | 0 | 0 | 1 |

### Top new findings (4 of 4)

| Severity | Location | Rule | Message |
|---|---|---|---|
| HIGH | `src/a/Order Service.java:3` | `java/self-assignment` | Remove this self-assignment of `total` \| it has no effect |
| MEDIUM | `src/a/Order Service.java:20` | `java/cognitive-complexity` | Reduce the cognitive complexity of `total` from 22 to 15 _(worsened)_ |
| MEDIUM | `src/a/Order Service.java:40` | `java/duplicated-block` | This block of 21 lines is duplicated in 1 other place |
| LOW | `src/c/Util.java:1` | `java/todo-comment` | Complete the task in this TODO comment |

### Measures

| Measure | New code | Overall |
|---|---:|---:|
| Lines | 38 | 300 |
| Duplication | 55.3% | 14.0% |
| Coverage | 80.0% | – |
| Estimated effort | 0.4 h | 0.9 h |
| Cognitive complexity | – | 57 |
| Cyclomatic complexity | – | 80 |
| Maintainability grade | **E** (8.77) | **D** (3.61) |
| Reliability grade | **E** (263.16) | **E** (41.67) |

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
