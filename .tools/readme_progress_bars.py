#!/usr/bin/env python3
"""Regenerate the per-phase progress bars in README.md and README.es.md.

The bars sit between `<!-- progress-bars:start -->` and `<!-- progress-bars:end -->`.
Only the analysis phase is read from the evidence ledger; the other values are measured
facts kept in PHASES below and must be edited together with the Completion plan table.

Usage: python3 .tools/readme_progress_bars.py [--check]
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEDGER = ROOT / "sheets/re/function_progress_evidence.tsv"
TOTAL = 153476
WIDTH = 20
START, END = "<!-- progress-bars:start -->", "<!-- progress-bars:end -->"


def ledger_counts():
    done = stubs = 0
    for ln in LEDGER.read_text().splitlines():
        if not ln.startswith("update_v262144_"):
            continue
        c = ln.split("\t")
        if len(c) > 5 and c[3] == "analyzed_documented":
            done += 1
            stubs += "PLT stub" in c[5]
    return done, stubs


def bar(pct):
    n = WIDTH if pct >= 100 else int(pct // (100 / WIDTH))
    return "█" * n + "░" * (WIDTH - n)


def fmt(pct, lang, est=False):
    if pct == 0:
        s = "0"
    elif pct < 0.1:
        s = "<0,1" if lang == "es" else "<0.1"
    else:
        s = f"{pct:.1f}" if pct < 99.95 and pct != int(pct) else f"{int(pct)}"
        if lang == "es":
            s = s.replace(".", ",")
    return ("~" if est else "") + s + " %"


def lines(lang):
    done, stubs = ledger_counts()
    es = lang == "es"

    def num(n):
        return f"{n:,}".replace(",", ".") if es else f"{n:,}"

    ap = 100 * done / TOTAL
    rows = [
        ("1", "Extracción" if es else "Extraction", 100, False,
         "archivos y miembros enumerados" if es else "enumerated files and members"),
        ("2", "Inventario de main" if es else "Inventory of main", 98.4, False,
         "bytes ejecutables dentro de funciones" if es else "executable bytes inside functions"),
        ("3", "Export de pseudocódigo" if es else "Pseudocode export", 100, False,
         "de las 153.476 funciones localizadas" if es else "of the 153,476 located functions"),
        ("4", "Índice gameDB" if es else "gameDB index", 100, False,
         "de los archivos C exportados" if es else "of the exported C files"),
        ("5", "Datos (RomFS)" if es else "Data (RomFS)", 100, False,
         "extraídos y superpuestos" if es else "extracted and overlaid"),
        ("", "  · interpretación" if es else "  · interpretation", 100 * 14 / 466, False,
         "14/466 modificados comparados por dentro" if es else "14/466 modified files compared inside"),
        ("6", "Análisis de funciones" if es else "Function analysis", ap, False,
         (f"{num(done)} de {num(TOTAL)} documentadas ({num(stubs)} son centralitas de importación)" if es
          else f"{num(done)} of {num(TOTAL)} documented ({num(stubs)} are import stubs)")),
        ("7", "Implementación (port)" if es else "Implementation (port)", 100 * 8 / TOTAL, False,
         "8 de 153.476 con implementación parcial" if es else "8 of 153,476 with a partial implementation"),
        ("8", "Verificación de conducta" if es else "Behaviour verification", 0, False, "0 de 153.476" if es else "0 of 153,476"),
        ("9", "Binary matching", 0, False, "0 de 153.476" if es else "0 of 153,476"),
        ("10", "Port jugable" if es else "Playable port", 45, True,
         "media de estimaciones cualitativas" if es else "average of qualitative estimates"),
    ]
    out = []
    for num, name, pct, est, note in rows:
        label = (f"{num:>2} " if num else "   ") + name
        out.append(f"{label:<28}{bar(pct)} {fmt(pct, lang, est):>8}   {note}")
    return out


def render(lang):
    es = lang == "es"
    cap = ("Las barras miden lo que está cuantificado en cada fase; el denominador completo de varias fases es desconocido (véase la Tabla de cierre). "
           "La fase 2 mide bytes de código cubiertos por funciones, no número de funciones; la fase 6 incluye centralitas de importación que no se han «leído»."
           if es else
           "Bars show what is actually quantified in each phase; several phases have an unknown full denominator (see the Completion plan). "
           "Phase 2 measures executable bytes covered by functions, not a function count; phase 6 includes import stubs that were not \"read\".")
    return "\n".join([START, "```text", *lines(lang), "```", "", f"*{cap}*", END])


def main():
    check = "--check" in sys.argv
    rc = 0
    for name, lang in (("README.md", "en"), ("README.es.md", "es")):
        p = ROOT / name
        t = p.read_text()
        if START not in t or END not in t:
            print(f"{name}: markers missing", file=sys.stderr)
            rc = 1
            continue
        new = re.sub(re.escape(START) + r".*?" + re.escape(END), lambda m: render(lang), t, flags=re.S)
        if new != t:
            if check:
                print(f"{name}: bars out of date")
                rc = 1
            else:
                p.write_text(new)
                print(f"{name}: bars updated")
        else:
            print(f"{name}: bars current")
    return rc


if __name__ == "__main__":
    sys.exit(main())
