#!/usr/bin/env python3
"""
Antigravity Lifecycle Hook: Compaction Guard
Enforces the Fresh Read Protocol defined in AGENTS.md.
Locks all mutation and execution tools after context compaction until task_plan.md is viewed.
"""

import argparse
import json
import os
import sys
from pathlib import Path

STATE_FILE = Path(__file__).resolve().parent.parent / ".guard_state.json"

def load_state() -> dict:
    if STATE_FILE.exists():
        try:
            with open(STATE_FILE, "r", encoding="utf-8") as file_handle:
                return json.load(file_handle)
        except Exception:
            pass
    return {
        "max_step_index": 0,
        "compaction_active": False,
        "last_task_plan_read_step": 0,
    }

def save_state(state: dict):
    try:
        STATE_FILE.parent.mkdir(parents=True, exist_ok=True)
        with open(STATE_FILE, "w", encoding="utf-8") as file_handle:
            json.dump(state, file_handle, indent=2)
    except Exception as error:
        sys.stderr.write(f"Failed to save guard state: {error}\n")

def inspect_transcript_for_compaction(transcript_path: str, state: dict) -> bool:
    if not transcript_path or not os.path.exists(transcript_path):
        return False

    compaction_detected = False
    max_seen_step = state.get("max_step_index", 0)
    current_max_step = 0
    recent_entries = []

    try:
        with open(transcript_path, "r", encoding="utf-8") as file_handle:
            for line in file_handle:
                line = line.strip()
                if not line:
                    continue
                try:
                    entry = json.loads(line)
                    step_index = entry.get("step_index", 0)
                    if step_index > current_max_step:
                        current_max_step = step_index
                    recent_entries.append(entry)
                    if len(recent_entries) > 30:
                        recent_entries.pop(0)
                except Exception:
                    continue
    except Exception as error:
        sys.stderr.write(f"Error reading transcript: {error}\n")
        return False

    # Check 1: Did step_index drop significantly below previous maximum seen?
    if max_seen_step > 0 and current_max_step < max_seen_step:
        compaction_detected = True

    # Check 2: Does recent entries contain <CONTEXT_SUMMARY>?
    last_compaction_step = 0
    for entry in recent_entries:
        content = entry.get("content", "")
        if "<CONTEXT_SUMMARY>" in content:
            last_compaction_step = max(last_compaction_step, entry.get("step_index", 0))

    if last_compaction_step > 0:
        last_read = state.get("last_task_plan_read_step", 0)
        if last_compaction_step >= last_read:
            compaction_detected = True

    state["max_step_index"] = max(max_seen_step, current_max_step)
    return compaction_detected

def handle_pre_invocation(payload: dict):
    state = load_state()
    transcript_path = payload.get("transcriptPath", "")

    compaction_detected = inspect_transcript_for_compaction(transcript_path, state)

    if compaction_detected:
        state["compaction_active"] = True
        save_state(state)
        response = {
            "injectSteps": [
                {
                    "ephemeralMessage": (
                        "🚨 MANDATORY DESIGN GUIDELINES & FRESH READ GATE 🚨\n"
                        "Context compaction was detected. In accordance with AGENTS.md, you MUST perform a fresh read "
                        "of ## Mandatory Design Guidelines & Engineering Standards in task_plan.md (lines 12-124) via view_file.\n"
                        "All mutation tools (write_to_file, replace_file_content, run_command) are LOCKED until task_plan.md is viewed."
                    )
                }
            ]
        }
    else:
        save_state(state)
        response = {}

    json.dump(response, sys.stdout)

def handle_pre_tool_use(payload: dict):
    state = load_state()
    tool_call = payload.get("toolCall", {})
    tool_name = tool_call.get("name", "")
    tool_args = tool_call.get("args", {})
    step_idx = payload.get("stepIdx", 0)

    # If calling view_file on task_plan.md, disarm the gate
    if tool_name == "view_file":
        absolute_path = str(tool_args.get("AbsolutePath", ""))
        if "task_plan.md" in absolute_path:
            state["compaction_active"] = False
            state["last_task_plan_read_step"] = step_idx
            save_state(state)
            json.dump({"decision": "allow"}, sys.stdout)
            return

    # Check transcript in case pre_invocation was bypassed or missed
    transcript_path = payload.get("transcriptPath", "")
    if inspect_transcript_for_compaction(transcript_path, state):
        state["compaction_active"] = True
        save_state(state)

    # Mutation and command execution tools
    mutation_tools = {
        "write_to_file",
        "replace_file_content",
        "run_command",
        "invoke_subagent",
    }

    if tool_name in mutation_tools and state.get("compaction_active", False):
        response = {
            "decision": "deny",
            "reason": (
                "MANDATORY PROTOCOL GATE LOCKED: Context compaction occurred. "
                "You are strictly forbidden from modifying files or running commands until you have executed "
                "view_file on task_plan.md (lines 12-124) to refresh the Mandatory Design Guidelines & Engineering Standards from disk."
            ),
        }
    else:
        response = {"decision": "allow"}

    json.dump(response, sys.stdout)

def main():
    parser = argparse.ArgumentParser(description="Antigravity Compaction Guard Hook")
    parser.add_argument(
        "--event",
        choices=["pre_invocation", "pre_tool_use", "post_tool_use"],
        required=True,
    )
    args = parser.parse_args()

    try:
        input_data = sys.stdin.read()
        payload = json.loads(input_data) if input_data.strip() else {}
    except Exception as error:
        sys.stderr.write(f"Failed to parse hook stdin JSON: {error}\n")
        payload = {}

    if args.event == "pre_invocation":
        handle_pre_invocation(payload)
    elif args.event == "pre_tool_use":
        handle_pre_tool_use(payload)
    else:
        # Default empty response for other events
        json.dump({}, sys.stdout)

if __name__ == "__main__":
    main()
