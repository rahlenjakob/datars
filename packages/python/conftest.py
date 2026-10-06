# Doctests run over src/datars; the widget module needs the optional anywidget extra.
import importlib.util

collect_ignore_glob = []
if importlib.util.find_spec("anywidget") is None:
    collect_ignore_glob.append("src/datars/widget.py")
