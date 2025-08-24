#!/usr/bin/env python3
import argparse
import re
from pathlib import Path

SKIP_PREFIX_RE = re.compile(r'^\s*\[skip\]\b', re.IGNORECASE)

def load_keys(path: Path) -> set[str]:
    """参照ファイル(list)を読み込み、空行と # から始まる行を除いた集合を返す。"""
    keys: set[str] = set()
    with path.open('r', encoding='utf-8', newline='') as f:
        for line in f:
            s = line.strip()
            if not s or s.startswith('#'):
                continue
            keys.add(s)
    return keys

def needs_skip(line: str, keys: set[str]) -> bool:
    """行が既に [skip] で始まっていなければ、参照集合に含まれるかを判定。"""
    if SKIP_PREFIX_RE.match(line):
        return False  # 既に [skip] 付与済み
    content = line.strip()
    return content in keys

def mark_file(src: Path, ref: Path, in_place: bool = False, out: Path | None = None) -> None:
    keys = load_keys(ref)
    lines_out: list[str] = []

    with src.open('r', encoding='utf-8', newline='') as f:
        for line in f:
            if needs_skip(line, keys):
                # 末尾の改行はそのまま保持
                nl = '\n' if line.endswith('\n') else ''
                core = line[:-1] if nl else line
                # 先頭に [skip] を付与（元の行はそのまま残す）
                lines_out.append(f"[skip] {core}{nl}")
            else:
                lines_out.append(line)

    if in_place:
        target = src
    else:
        if out is None:
            # 標準出力
            import sys
            sys.stdout.writelines(lines_out)
            return
        target = out

    with target.open('w', encoding='utf-8', newline='') as f:
        f.writelines(lines_out)

def main():
    p = argparse.ArgumentParser(description="Prefix lines that appear in ref file with [skip].")
    p.add_argument("source", type=Path, help="対象ファイル（1行ずつチェック）")
    p.add_argument("ref", type=Path, help="参照ファイル（ここに載っている行は [skip] を付与）")
    p.add_argument("-i", "--in-place", action="store_true", help="source を上書きする")
    p.add_argument("-o", "--out", type=Path, help="出力先パス（未指定なら標準出力）")
    args = p.parse_args()

    mark_file(args.source, args.ref, in_place=args.in_place, out=args.out)

if __name__ == "__main__":
    main()
