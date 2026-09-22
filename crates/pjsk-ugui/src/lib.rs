//! # pjsk-ugui — Unity UGUI 布局求值器
//!
//! Q9 选了全分辨率范围，这里是其成本所在。**全部是确定的数学，无未知数。**
//!
//! ## 职责
//! - `RectTransform` 递归求值（anchor / pivot / offset / sizeDelta / scale / rotation）
//! - `CanvasScaler` 三种模式，含 `scaleFactor = 2^lerp(log2(w/refW), log2(h/refH), match)`
//! - LayoutGroup 家族的两趟布局、ContentSizeFitter、LayoutElement、AspectRatioFitter
//! - safe area 与 aspect clamp
//!
//! ## 不负责
//! - 绘制；文本排版（那是 `pjsk-text`）
//!
//! ## 测试
//! 对着 `tools/dumper` 在多分辨率下导出的 fixture 做断言，精确到浮点。
