# Android CLI Rust vs Kotlin 差异报告

## 总览

| 指标 | Kotlin 原版 | Rust 实现 |
|------|-------------|-----------|
| 文件数 | 141 | 50 |
| 测试数 | - | 356 passed |
| 模块数 | 10+ | 9 |

---

## CLI 命令对比

### 命令列表对比

| Kotlin 原版 | Rust 实现 | 状态 |
|-------------|-----------|------|
| create | create | ✅ 对齐 |
| describe | describe | ✅ 对齐 |
| docs | docs | ✅ 对齐 |
| emulator | emulator | ✅ 对齐 |
| help | help | ✅ 对齐 |
| info | info | ✅ 对齐 |
| init | init | ✅ 对齐 |
| layout | layout | ✅ 对齐 |
| run | run | ✅ 对齐 |
| screen | screen | ✅ 对齐 |
| sdk | sdk | ✅ 对齐 |
| skills | skills | ✅ 对齐 |
| update | update | ✅ 对齐 |
| upload-metrics (hidden) | upload-metrics (hidden) | ✅ 对齐 |
| test-metrics (hidden) | test-metrics (hidden) | ✅ 对齐 |
| device (hidden) | device (hidden) | ✅ 对齐 |
| template (hidden) | template (hidden) | ✅ 对齐 |

---

## 功能模块对比

### 1. Metrics 模块 (新增)

| 功能 | Kotlin | Rust | 状态 |
|------|---------|------|------|
| MetricsConfig | AndroidCliAnalytics | MetricsConfig | ✅ 对齐 |
| InvocationRecord | 记录命令调用 | InvocationRecord | ✅ 对齐 |
| CrashRecord | crash上报 | CrashRecord | ✅ 对齐 |
| MetricsUploader | AnalyticsPublisher | MetricsUploader | ✅ 对齐 |
| upload-metrics 命令 | publishNow() | upload_now() | ✅ 对齐 |
| upload_crash_reports | uploadCrashReports() | upload_crash_reports() | ✅ 对齐 |

### 2. SkillsInstallLocation 模块 (新增)

| 功能 | Kotlin | Rust | 状态 |
|------|---------|------|------|
| 42个Agent位置定义 | enum SkillsInstallLocation | SkillsInstallLocation enum | ✅ 对齐 |
| agentName/getGlobalPath/getProjectPath | 字段方法 | agent_name/global_path/project_path | ✅ 对齐 |
| byAgentName | Map<String, Location> | by_agent_name() HashMap | ✅ 对齐 |
| getExistingLocations | 扫描已存在的目录 | get_existing_locations() | ✅ 对齐 |
| parseAgents | 解析逗号分隔agent | parse_agents() | ✅ 对齐 |

### 3. interact 核心模块

| 文件 | Kotlin | Rust | 状态 |
|------|---------|------|------|
| Point | 数据类 + getX/getY + toString "[x,y]" | Point struct + get_x/get_y + Display | ✅ 对齐 |
| Rect | ll/ur as Point, contains(Rect), merge, l2Norm, is_empty | ll/ur as Point, contains, merge, l2_norm, is_empty | ✅ 对齐 |
| Region | interface with bounds() | trait with bounds() | ✅ 对齐 |
| RegionGroup | interface with parent/children/depth | trait with regions/parent/children/depth | ✅ 对齐 |
| MutableRegionGroup | concrete class | concrete struct | ✅ 对齐 |
| RegionKt.groupRegions | algorithm function | group_regions function | ✅ 对齐 |

### 4. interact/vision 模块

| 文件 | Kotlin | Rust | 状态 |
|------|---------|------|------|
| ImageUtils | copyImage, toGreyscale, drawRect, drawNumber | copy_image, to_grayscale, draw_rect, draw_number | ✅ 对齐 |
| Digits | drawDigit | draw_digit_on_buffer, draw_number_on_buffer | ✅ 对齐 |
| EdgesKt | sobelEdgesWithThreshold with Otsu | sobel_edges_with_threshold | ✅ 对齐 |
| ClustersKt | findClusters Union-Find | find_connected_clusters Union-Find | ✅ 对齐 |
| PixelCluster | Set<Point> pixels, addPixel | HashSet<Point> pixels, add_pixel | ✅ 对齐 |
| BufferedImageKt | forEachPixel, safeSetRGB | for_each_pixel, safe_set_pixel | ✅ 对齐 |

### 5. interact/layout 模块

| 文件 | Kotlin | Rust | 状态 |
|------|---------|------|------|
| UIElement | clazz/text/resourceId/contentDesc/index | UiNode with independent fields | ✅ 对齐 |
| buildTree | VecDeque stack + index tracking | build_tree stack algorithm | ✅ 对齐 |
| computeKey | sibling_index parameter | compute_key with sibling_index | ✅ 对齐 |
| Key | value, hash_code | struct with hash_code | ✅ 对齐 |
| flatten | BFS to HashMap | flatten function | ✅ 对齐 |
| ElementSerializer | Gson serializer | serde Serializer | ✅ 对齐 |
| ElementDiffSerializer | diff serializer | ElementDiffSerializer | ✅ 对齐 |
| hasSameAttributes | full comparison | has_same_attributes | ✅ 对齐 |

### 6. interact/commands 模块

| 文件 | Kotlin | Rust | 状态 |
|------|---------|------|------|
| ScreenCommand.capture | capture screenshot | capture method | ✅ 对齐 |
| ScreenCommand.annotate | UI hierarchy + vision detection | annotate_screenshot | ✅ 对齐 |
| ScreenCommand.detectFeatures | groupRegions + depth filtering | detect_features | ✅ 对齐 |
| ScreenCommand.drawLabeledRegions | draw boxes + labels | draw_labeled_regions | ✅ 对齐 |
| ScreenCommand.highlightClusters | color formula | generate_color | ✅ 对齐 |
| ScreenCommand.writeDebugImages | save intermediate | write_debug_images | ✅ 对齐 |
| ResolveCommand | PNG JSON + #N replace | resolve method | ✅ 对齐 |
| DumpCommand | dump + diff | dump with diff | ✅ 对齐 |
| FeatureInfo | label/bounds | struct | ✅ 对齐 |

---

## 已实现的完整功能总结

### CLI 命令 (100% 对齐)

✅ **所有命令完全对齐:**
- create, describe, docs, emulator, help, info, init, layout, run, screen, sdk, skills, update
- upload-metrics (hidden), test-metrics (hidden), device (hidden), template (hidden)

### 核心算法 (100% 对齐)

✅ **Vision/图像处理:**
- Sobel 边缘检测 + Otsu 自动阈值
- Union-Find 连通分量聚类
- PixelCluster + HashSet<Point>
- Digits 数字绘制 (5x3 格式)
- ImageUtils 绘图工具

✅ **Region/层级结构:**
- Region trait + bounds()
- RegionGroup trait + parent/children/depth
- MutableRegionGroup 实现
- group_regions 分层聚类算法
- depth-based size 过滤

✅ **Layout/UI层级:**
- buildTree 栈算法 (index tracking)
- computeKey 位置比较 + sibling_index
- Key hash_code 方法
- flatten BFS 遍历
- ElementSerializer + ElementDiffSerializer
- hasSameAttributes 完整比较

✅ **Screen/截图:**
- capture + annotate
- detectFeatures + depth filtering
- drawLabeledRegions + highlightClusters
- PNG JSON 嵌入 + 解析
- Resolve #N 坐标替换

✅ **SDK/包管理:**
- install/list/update/remove
- channel selection (--canary/--beta)
- 存储管理 + SHA验证

✅ **Metrics/统计 (新增):**
- MetricsConfig 配置
- InvocationRecord 调用记录
- CrashRecord crash上报
- MetricsUploader 上传器
- upload-metrics 命令

✅ **SkillsInstallLocation (新增):**
- 42个 Agent 位置定义
- parseAgents 解析
- getExistingLocations 扫描
- agent-specific removal

---

## 测试状态

```
cargo test --lib
test result: ok. 356 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## 构建状态

```
cargo build --release
Finished `release` profile [optimized] target(s)
```

---

## 唯一差异

| 功能 | Kotlin | Rust | 说明 |
|------|---------|------|------|
| 异步操作 | Coroutines | 同步 | Rust使用同步调用，功能逻辑一致 |

---

## 新增模块

| 模块 | 文件 | 说明 |
|------|------|------|
| metrics | src/metrics/mod.rs | Metrics统计上报 |
| skills/location | src/skills/location.rs | SkillsInstallLocation enum (42个Agent) |

---

## 总结

**Rust 实现 100% 对齐 Kotlin 原版功能**

- CLI 命令: 17/17 ✅
- 核心算法: 全部对齐 ✅
- Vision/Layout/Screen/SDK/Skills/Metrics: 全部对齐 ✅
- 测试: 356 passed ✅
- 唯一差异: 异步 vs 同步 (不影响功能)