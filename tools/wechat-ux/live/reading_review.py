#!/usr/bin/env python3
"""Build a local before/after review from untouched native screenshots."""
import argparse
import html
import json
import shutil
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before', type=Path, required=True)
    parser.add_argument('--after', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    before = json.loads(args.before.read_text())
    after = json.loads(args.after.read_text())
    rows, panels = [], []
    old = {s['name']: s for s in before['screens']}
    for screen in after['screens']:
        name = screen['name']
        previous = old.get(name)
        rows.append(f'<tr><td>{name}</td><td>{previous["score"]["total"] if previous else "—"}</td>'
                    f'<td>{screen["score"]["total"]}</td><td>{min((c["ratio"] for c in screen.get("text_contrast",[])),default=0)}:1</td></tr>')
        figures = []
        for label, state in [('修改前', previous), ('修改后', screen)]:
            if not state: continue
            target = f'{name}-{label}.png'
            shutil.copyfile(state['path'],args.output/target)
            figures.append(f'<figure><figcaption>{label}</figcaption><a href="{target}"><img loading="lazy" src="{target}" alt="{name} {label}"></a></figure>')
        panels.append(f'<section data-name="{name}"><h3>{name}</h3><div class="pair">{"".join(figures)}</div></section>')
    checks = sum(len(v) for v in after.get('journeys',{}).values())
    details = html.escape(json.dumps(after.get('journeys',{}),ensure_ascii=False,indent=2))
    page = '''<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Rinx 阅读体验对标审查</title><style>
:root{font:16px/1.7 system-ui;color:#18232d;background:#f0f3f6}body{max-width:1120px;margin:48px auto;padding:0 24px}h1{font-size:34px;line-height:1.3}h2{margin-top:40px}a{color:#006c7a}p{max-width:80ch}.status{padding:16px 20px;background:#fff1de;border-left:4px solid #92581d;border-radius:8px}table{width:100%;border-collapse:collapse;background:white}th,td{padding:14px;text-align:left;border-bottom:1px solid #d6dee6;vertical-align:top}th{background:#e4ebf1}.pair{display:grid;grid-template-columns:1fr 1fr;gap:24px}figure{margin:0}figcaption{font-weight:600;padding:8px 0}img{display:block;width:100%;height:auto;border:1px solid #ccd6df}section{margin:36px 0}section[data-name*="mobile"] .pair{max-width:780px}button{min-height:44px;padding:8px 16px;border:1px solid #b3c1cc;border-radius:8px;background:#fff;font:inherit;cursor:pointer}button[aria-pressed="true"]{background:#006c7a;color:white}nav{display:flex;gap:8px;flex-wrap:wrap;position:sticky;top:0;padding:12px 0;background:#f0f3f6}pre{overflow:auto;padding:20px;background:white;font-size:13px}small{color:#4f5c68}@media(max-width:600px){body{padding:0 16px;margin:24px auto}.pair{gap:12px}h1{font-size:26px}table{font-size:13px}td,th{padding:8px}}
</style><h1>朋友圈 · 公众号 · 标签阅读器</h1>
<p>审查范围：桌面原生窗口与 390px 手机宽度窗口，浅色和暗色。使用真实生产组件与离线测试内容，没有读取用户聊天或伪造订阅数据。</p>
<p class="status"><strong>95 分门槛：未通过。</strong>局部修改和操作测试不能证明完整微信体验达到 95%。公众号订阅流尚不存在，也没有本轮手机真机证据。</p>
<h2>差距与设计决定</h2><table><thead><tr><th>部分</th><th>原问题</th><th>本次实现</th><th>仍须完成</th></tr></thead><tbody>
<tr><td>朋友圈</td><td>首屏堆放管理操作；头像颜色单一；点赞评论只有数量；图片一律三列。</td><td>发布与更多操作分层；头像使用成员照片与稳定的个人颜色；显示点赞人和评论预览；1/2/4/9 图按数量排版。</td><td>个人封面图、直接展开长文、信息流内快捷回复；手机真机触控验证。</td></tr>
<tr><td>公众号</td><td>自己的草稿库不能承担关注账号、接收更新和连续阅读的任务；状态分类仅靠图标。</td><td>保留准确的“我的文章”名称；有名称和选中状态的分类；标题、摘要、状态、真实封面分层。</td><td>公众号身份页、关注/取消关注、订阅更新流及其 Matrix 数据模型。不能用一个改名页面冒充已实现。</td></tr>
<tr><td>标签阅读器</td><td>标签、工具栏、正文重复标题；标题固定占用视口；按钮过小。</td><td>文章只保留标签和随正文滚动的标题；有界阅读宽度；常驻返回；44px 控件；Cmd/Ctrl+W 与 Ctrl+Tab。</td><td>当前网页地址的 SDK 回传、手机多标签概览、中文标点禁则与更细的排版节奏。</td></tr>
<tr><td>共同基础</td><td>系统中文字体声明被延迟加载覆盖；暗色卡片仍是白色。</td><td>系统字体作为真实字体家族成员注册；动态卡片颜色跟随主题；实际文字区域对比度检查。</td><td>完整真机矩阵与独立参考图审查。</td></tr></tbody></table>
<h2>参照来源与证据边界</h2>
<p><a href="https://www.ithome.com/0/842/416.htm">2025-04-01 桌面朋友圈与个人相册</a>；
<a href="https://www.cnbeta.com.tw/articles/tech/1452102.htm">2024-11-03 桌面公众号截图</a>；
<a href="https://news.mydrivers.com/1/897/897585.htm">2023-03-15 手机订阅号截图</a>；
<a href="https://www.ithome.com/0/926/302.htm">2026-03-05 桌面阅读布局变化</a>。
这些公开资料版本不同；手机资料较旧，不能作为当前微信真机的逐像素对照。本机安装了微信，但本轮没有采集其私人页面。</p>
<h2>评分与必过检查</h2><p>下表保留原始像素评分器结果（满分 10），它衡量像素分布，<strong>不是与微信的相似度百分比</strong>。评分器曾给白底浅字接近满分，因此任何不可读区域、功能缺口、操作失败或未经验证的设备状态均不能因总分高而豁免。报告的原生截图未经裁剪或修图。</p>
<table><thead><tr><th>窗口 / 主题</th><th>修改前像素分</th><th>修改后像素分</th><th>本次实测最低文字对比度</th></tr></thead><tbody>ROWS</tbody></table>
<p>本报告记录 CHECKS 项离线原生操作断言。授权、加密、真实订阅投递与手机真机测试不在这些断言中；测试构建未优化，不能用其耗时宣称生产性能。</p>
<details><summary>查看操作检查</summary><pre>DETAILS</pre></details>
<h2>原生截图对比</h2><nav aria-label="截图筛选"><button aria-pressed="true" data-filter="">全部</button><button aria-pressed="false" data-filter="moments">朋友圈</button><button aria-pressed="false" data-filter="library">我的文章</button><button aria-pressed="false" data-filter="reader">阅读器</button></nav>PANELS
<script>document.querySelectorAll('button[data-filter]').forEach(b=>b.onclick=()=>{document.querySelectorAll('button[data-filter]').forEach(x=>x.setAttribute('aria-pressed',String(x===b)));document.querySelectorAll('section[data-name]').forEach(s=>s.hidden=!s.dataset.name.includes(b.dataset.filter))})</script></html>'''
    page = page.replace('ROWS',''.join(rows)).replace('CHECKS',str(checks)).replace('DETAILS',details).replace('PANELS',''.join(panels))
    (args.output/'index.html').write_text(page)
    shutil.copyfile(args.after,args.output/'report.json')
    print(args.output/'index.html')


if __name__ == '__main__':
    main()
