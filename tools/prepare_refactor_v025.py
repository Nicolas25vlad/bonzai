from pathlib import Path

path = Path("tools/refactor_v025.py")
s = path.read_text()

start_marker = "s = replace_once(\n    s,\n    '''{TITLE}INFO{RESET}"
end_marker = '    "usage update command",\n)\n\n'

start = s.find(start_marker)
if start != -1:
    end = s.find(end_marker, start)
    if end == -1:
        raise SystemExit("could not locate end of optional usage patch")
    s = s[:start] + s[end + len(end_marker):]

path.write_text(s)
