#!/usr/bin/env python3
"""ChatLingo 发版脚本（POLISH_DESIGN §2.4 五步流程）

用法：
  python scripts/release.py --notes release-notes.md [--draft]

步骤：
  1. 版本三处 bump 校验一致（tauri.conf.json / package.json / Cargo.toml）
  2. 构建前杀进程 → npm run tauri build（带签名环境变量）
  3. 生成 latest.json（版本/notes/setup.exe 的 .sig）
  4. 创建 GitHub Release + 上传 setup.exe / chatlingo.exe / latest.json
     （大文件一律 curl —— urllib 408 教训，HANDOFF 陷阱 19）
  5. 归档产物到 D:\\hobby\\产物\\chatlingo\\<版本>\\

签名私钥与密码从 D:\\hobby\\产物\\chatlingo\\signing\\ 读取（不入库）。
GitHub token 从 ~/.git-credentials 解出（username rem-sy）。
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import datetime
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SRC_TAURI = REPO / "src-tauri"
TARGET = Path(r"C:\chatlingo-target\release")
SIGNING_DIR = Path(r"D:\hobby\产物\chatlingo\signing")
ARCHIVE_ROOT = Path(r"D:\hobby\产物\chatlingo")
REPO_URL = "https://github.com/rem-sy/chatlingo"
PROXY = "http://127.0.0.1:7890"


def die(msg: str):
    print(f"[release] FAIL: {msg}", file=sys.stderr)
    sys.exit(1)


def run(cmd, **kw) -> subprocess.CompletedProcess:
    print(f"[release] $ {' '.join(str(c) for c in cmd) if isinstance(cmd, list) else cmd}")
    return subprocess.run(cmd, check=True, shell=isinstance(cmd, str), **kw)


def curl(args, output=None, input_str=None, check=True):
    """统一 curl 封装：带代理回退（Clash 在线走代理，否则直连）"""
    base = ["curl", "-sf", "--connect-timeout", "10"]
    # 先探测代理可用性
    probe = subprocess.run(
        ["curl", "-sf", "--connect-timeout", "3", "-x", PROXY, "https://api.github.com/zen"],
        capture_output=True,
    )
    if probe.returncode == 0:
        base += ["-x", PROXY]
    cmd = base + args
    if output:
        cmd += ["-o", str(output)]
    print(f"[release] $ {' '.join(str(c) for c in cmd)}")
    r = subprocess.run(cmd, capture_output=True, **({"input": input_str} if input_str else {}))
    if isinstance(r.stdout, bytes):
        r.stdout = r.stdout.decode("utf-8", "replace")
    if isinstance(r.stderr, bytes):
        r.stderr = r.stderr.decode("utf-8", "replace")
    if check and r.returncode != 0:
        die(f"curl 失败: {r.stderr}")
    return r


def github_token() -> str:
    # 格式 https://rem-sy:<token>@github.com
    cred = Path.home() / ".git-credentials"
    if not cred.exists():
        die("~/.git-credentials 不存在，无法取 GitHub token")
    t = cred.read_text().strip()
    m = re.search(r"github\.com:[^:]+:([^@\s]+)@", t) or re.search(r":(gh[a-z]_[A-Za-z0-9_]+)@", t)
    if m:
        return m.group(1)
    die("git-credentials 中未找到 GitHub token")


# ---------- 步骤 1：版本三处校验 ----------

def check_versions() -> str:
    conf = json.loads((SRC_TAURI / "tauri.conf.json").read_text(encoding="utf-8"))
    v1 = conf["version"]
    pkg = json.loads((REPO / "package.json").read_text(encoding="utf-8"))
    v2 = pkg["version"]
    v3 = re.search(r'^version = "(.+)"', (SRC_TAURI / "Cargo.toml").read_text(encoding="utf-8"), re.M).group(1)
    if not (v1 == v2 == v3):
        die(f"版本三处不一致: tauri.conf={v1} package.json={v2} Cargo.toml={v3}")
    print(f"[release] 版本一致: {v1}")
    return v1


# ---------- 步骤 2：构建 ----------

def build(version: str):
    subprocess.run(
        ["powershell", "-NoProfile", "-Command",
         "Get-Process chatlingo -ErrorAction SilentlyContinue | Stop-Process -Force"],
        check=False,
    )
    key = (SIGNING_DIR / "chatlingo.key").read_text(encoding="utf-8").strip()
    password = (SIGNING_DIR / "chatlingo.key.password.txt").read_text(encoding="utf-8").strip()
    env = dict(os.environ)
    env["TAURI_SIGNING_PRIVATE_KEY"] = key
    env["TAURI_SIGNING_PRIVATE_KEY_PASSWORD"] = password
    run("npm run tauri build", cwd=REPO, env=env)
    setup = TARGET / "bundle" / "nsis" / f"ChatLingo_{version}_x64-setup.exe"
    sig = Path(str(setup) + ".sig")
    exe = TARGET / "chatlingo.exe"
    for f in (setup, sig, exe):
        if not f.exists():
            die(f"构建产物缺失: {f}")
    print(f"[release] 构建产物 OK: {setup.name} ({setup.stat().st_size/1048576:.1f}MB), chatlingo.exe ({exe.stat().st_size/1048576:.1f}MB)")
    return setup, sig, exe


# ---------- 步骤 3：latest.json ----------

def make_latest_json(version: str, notes: str, sig: Path, setup: Path) -> str:
    asset_url = f"{REPO_URL}/releases/download/v{version}/{setup.name}"
    return json.dumps({
        "version": version,
        "notes": notes,
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": {
            "windows-x86_64": {
                "signature": sig.read_text(encoding="utf-8").strip(),
                "url": asset_url,
            }
        },
    }, ensure_ascii=False, indent=2)


# ---------- 步骤 4：Release ----------

def create_release(version: str, notes: str, setup: Path, exe: Path, latest_json: str, draft: bool):
    token = github_token()
    tag = f"v{version}"
    api = "https://api.github.com/repos/rem-sy/chatlingo/releases"
    # 查重：同 tag 已有 release 则报错（防覆盖）
    exist = curl(["-H", f"Authorization: token {token}", f"{api}/tags/{tag}"], check=False)
    if exist.returncode == 0 and '"id"' in exist.stdout:
        die(f"Release {tag} 已存在，先手动删除或改版本号")
    body = json.dumps({
        "tag_name": tag,
        "name": f"ChatLingo v{version}",
        "body": notes,
        "draft": draft,
        "prerelease": False,
    }, ensure_ascii=False)
    r = curl(
        ["-H", f"Authorization: token {token}",
         "-H", "Content-Type: application/json; charset=utf-8",
         "-X", "POST", "-d", "@-", api],
        input_str=body.encode("utf-8"),
    )
    rel = json.loads(r.stdout)
    upload = rel.get("upload_url", "").split("{")[0]
    if not upload:
        die(f"创建 Release 失败: {r.stdout[:500]}")
    print(f"[release] Release 创建成功 id={rel['id']} draft={draft}")
    for path in (setup, exe):
        curl([
            "-H", f"Authorization: token {token}",
            "-H", "Content-Type: application/octet-stream",
            "-X", "POST", "--data-binary", f"@{path}",
            f"{upload}?name={path.name}",
        ])
        print(f"[release] 上传完成: {path.name}")
    # latest.json 小文件也走 curl，保持一致
    tmp = Path(os.environ.get("TEMP", ".")) / "chatlingo-latest.json"
    tmp.write_text(latest_json, encoding="utf-8")
    curl([
        "-H", f"Authorization: token {token}",
        "-H", "Content-Type: application/json",
        "-X", "POST", "--data-binary", f"@{tmp}",
        f"{upload}?name=latest.json",
    ])
    print("[release] 上传完成: latest.json")


# ---------- 步骤 5：归档 ----------

def archive(version: str, setup: Path, exe: Path):
    dst = ARCHIVE_ROOT / version
    dst.mkdir(parents=True, exist_ok=True)
    shutil.copy2(setup, dst / setup.name)
    shutil.copy2(exe, dst / "chatlingo.exe")
    sig = Path(str(setup) + ".sig")
    shutil.copy2(sig, dst / sig.name)
    print(f"[release] 归档完成: {dst}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--notes-file", required=True, help="Release notes markdown 文件（UTF-8）")
    ap.add_argument("--draft", action="store_true", help="创建 draft Release（先不上线验证）")
    ap.add_argument("--skip-build", action="store_true", help="跳过构建（产物已就绪时）")
    args = ap.parse_args()

    notes = Path(args.notes_file).read_text(encoding="utf-8").strip()
    version = check_versions()
    if args.skip_build:
        setup = TARGET / "bundle" / "nsis" / f"ChatLingo_{version}_x64-setup.exe"
        exe = TARGET / "chatlingo.exe"
    else:
        setup, _, exe = build(version)
    latest = make_latest_json(version, notes, Path(str(setup) + ".sig"), setup)
    create_release(version, notes, setup, exe, latest, args.draft)
    archive(version, setup, exe)
    print(f"[release] v{version} 发版流程全部完成")


if __name__ == "__main__":
    main()
