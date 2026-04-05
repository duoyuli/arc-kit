#!/usr/bin/env python3
"""Append structured task logs with file locking."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import time
from pathlib import Path

TZ_CST = dt.timezone(dt.timedelta(hours=8))
ALLOWED_STATUS = {"received", "in-progress", "blocked", "submitted"}


def now_iso() -> str:
    return dt.datetime.now(TZ_CST).replace(microsecond=0).isoformat()


def _is_stale_lock(lock_path: Path) -> bool:
    try:
        pid = int(lock_path.read_text().strip())
        os.kill(pid, 0)
        return False
    except (ValueError, ProcessLookupError, PermissionError, OSError):
        return True


def acquire_lock(lock_path: Path, timeout_sec: int) -> None:
    deadline = time.time() + timeout_sec
    while True:
        try:
            fd = os.open(str(lock_path), os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            os.write(fd, str(os.getpid()).encode("utf-8"))
            os.close(fd)
            return
        except FileExistsError:
            if _is_stale_lock(lock_path):
                try:
                    lock_path.unlink()
                except FileNotFoundError:
                    pass
                continue
            if time.time() >= deadline:
                raise TimeoutError(f"timeout waiting lock: {lock_path}")
            time.sleep(0.1)


def release_lock(lock_path: Path) -> None:
    try:
        lock_path.unlink()
    except FileNotFoundError:
        pass


def build_block(
    ts: str,
    agent: str,
    task_id: str,
    status: str,
    summary: str,
    output: str,
    next_step: str,
    blockers: str,
) -> str:
    return (
        f"- {ts} | agent={agent} | task={task_id} | status={status}\n"
        f"  - summary: {summary}\n"
        f"  - output: {output}\n"
        f"  - next: {next_step}\n"
        f"  - blockers: {blockers}\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Append one structured event to tasks-log.md with lock."
    )
    parser.add_argument(
        "--task-dir", required=True, help="e.g. ~/.openclaw/tasks/<slug>"
    )
    parser.add_argument("--agent", required=True)
    parser.add_argument("--task", required=True, dest="task_id")
    parser.add_argument("--status", required=True, choices=sorted(ALLOWED_STATUS))
    parser.add_argument("--summary", required=True)
    parser.add_argument("--output", default="n/a")
    parser.add_argument("--next", dest="next_step", default="n/a")
    parser.add_argument("--blockers", default="none")
    parser.add_argument(
        "--time",
        dest="event_time",
        help="override timestamp, ISO 8601 e.g. 2026-03-14T10:30:00+08:00",
    )
    parser.add_argument("--lock-timeout-sec", type=int, default=30)
    args = parser.parse_args()

    task_dir = Path(args.task_dir).expanduser().resolve()
    if not task_dir.exists():
        raise SystemExit(f"task dir not found: {task_dir}")

    log_path = task_dir / "tasks-log.md"
    lock_path = task_dir / "tasks-log.lock"
    if not log_path.exists():
        raise SystemExit(f"missing file: {log_path}")

    ts = args.event_time.strip() if args.event_time else now_iso()
    block = build_block(
        ts=ts,
        agent=args.agent.strip(),
        task_id=args.task_id.strip(),
        status=args.status,
        summary=args.summary.strip(),
        output=args.output.strip() or "n/a",
        next_step=args.next_step.strip() or "n/a",
        blockers=args.blockers.strip() or "none",
    )

    acquire_lock(lock_path, args.lock_timeout_sec)
    try:
        with log_path.open("a", encoding="utf-8") as f:
            f.write(block)
    finally:
        release_lock(lock_path)

    print(
        json.dumps(
            {
                "ok": True,
                "log": str(log_path),
                "agent": args.agent,
                "task": args.task_id,
                "status": args.status,
                "time": ts,
            },
            ensure_ascii=False,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
