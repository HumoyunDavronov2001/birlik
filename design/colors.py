"""BIRLIK design tokens.

One source (OKLCH values below) -> every platform:
  tokens.json                     W3C DTCG tokens
  tokens.css                      CSS custom properties (dark default, light via media query or data-theme)
  colors.json                     resolved hex values (used by the concept page)
  platforms/BirlikTokens.swift    SwiftUI
  platforms/BirlikTokens.kt       Jetpack Compose
  platforms/tokens.ts             TypeScript

It also enforces the contrast gate: the script exits with status 1 when any text color falls below WCAG AA
(4.5:1) or a graphic color below 3:1 on the surfaces it is used on.

Usage: python3 design/colors.py
"""
import json
import math
import pathlib
import sys

HERE = pathlib.Path(__file__).parent


def oklch_to_srgb(L, C, h):
    a = C * math.cos(math.radians(h))
    b = C * math.sin(math.radians(h))
    l_ = L + 0.3963377774 * a + 0.2158037573 * b
    m_ = L - 0.1055613458 * a - 0.0638541728 * b
    s_ = L - 0.0894841775 * a - 1.2914855480 * b
    l, m, s = l_ ** 3, m_ ** 3, s_ ** 3
    r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s
    g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s
    bb = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s

    def encode(x):
        x = max(0.0, min(1.0, x))
        return 12.92 * x if x <= 0.0031308 else 1.055 * x ** (1 / 2.4) - 0.055

    return tuple(encode(v) for v in (r, g, bb))


def to_hex(rgb):
    return "#%02x%02x%02x" % tuple(round(v * 255) for v in rgb)


def luminance(hx):
    v = [int(hx[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    v = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in v]
    return 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2]


def contrast(a, b):
    la, lb = sorted([luminance(a), luminance(b)], reverse=True)
    return (la + 0.05) / (lb + 0.05)


def blend(fg, bg, alpha):
    """Composite a translucent layer over its background before measuring contrast."""
    f = [int(fg[i:i + 2], 16) for i in (1, 3, 5)]
    g = [int(bg[i:i + 2], 16) for i in (1, 3, 5)]
    return "#%02x%02x%02x" % tuple(round(alpha * x + (1 - alpha) * y) for x, y in zip(f, g))


# Palette taken from Samarkand tilework: lapis, turquoise, gold and ganch (white plaster).
THEMES = {
    "dark": {  # default theme
        "bg":               (0.145, 0.022, 262),
        "surface-1":        (0.185, 0.024, 262),
        "surface-2":        (0.225, 0.026, 262),
        "surface-3":        (0.265, 0.028, 262),
        "text-1":           (0.965, 0.006, 262),
        "text-2":           (0.815, 0.018, 262),
        "text-3":           (0.690, 0.022, 262),
        "turquoise":        (0.800, 0.125, 190),   # the single accent
        "turquoise-strong": (0.700, 0.120, 190),
        "gold":             (0.835, 0.135, 84),    # text-safe gold (credits)
        "gold-mark":        (0.835, 0.135, 84),    # graphic gold: tokens, logo dot
        "lapis":            (0.650, 0.160, 266),
    },
    "light": {  # "ganch" paper
        "bg":               (0.983, 0.005, 85),
        "surface-1":        (1.000, 0.000, 0),
        "surface-2":        (0.962, 0.006, 85),
        "surface-3":        (0.935, 0.008, 85),
        "text-1":           (0.205, 0.032, 262),
        "text-2":           (0.400, 0.030, 262),
        "text-3":           (0.505, 0.026, 262),
        "turquoise":        (0.505, 0.095, 192),
        "turquoise-strong": (0.450, 0.090, 192),
        "gold":             (0.535, 0.118, 72),
        "gold-mark":        (0.640, 0.150, 74),    # graphic only: 3.3:1 on paper (>= 3:1), not for text
        "lapis":            (0.430, 0.175, 266),
    },
}

# Status colors are fixed across themes and always ship with an icon and a label.
STATUS = {"good": "#0ca30c", "warning": "#fab219", "serious": "#ec835a", "critical": "#d03b3b"}
# Chart colors: the first three slots of the validated data-viz palette (pass all-pairs CVD checks in both themes).
CHART = {
    "dark":  {"network": "#3987e5", "draft": "#d95926", "compute": "#199e70"},
    "light": {"network": "#2a78d6", "draft": "#eb6834", "compute": "#1baf7a"},
}
# Hairlines: a translucent layer of ink over the surface (hex, alpha).
LINES = {
    "dark":  {"line": ("#ffffff", 0.08), "line-2": ("#ffffff", 0.14)},
    "light": {"line": ("#0f1726", 0.10), "line-2": ("#0f1726", 0.16)},
}
FONT_SIZE = {"micro": 11, "small": 12, "caption": 13, "base": 14, "body": 16,
             "heading-3": 20, "heading-2": 28, "heading-1": 40, "display": 56}
SPACE = (4, 8, 12, 16, 24, 32, 48, 64)
RADIUS = {"small": 6, "medium": 10, "large": 16, "full": 999}
MOTION_MS = {"fast": 120, "base": 200, "slow": 320, "scene": 480}
EASING = (0.2, 0.8, 0.2, 1)

# Contrast gate: (foreground token, minimum ratio) checked on every listed surface, in both themes.
TEXT_TOKENS = ("text-1", "text-2", "text-3", "turquoise", "gold", "lapis")
GRAPHIC_TOKENS = ("gold-mark",)
GATE_SURFACES = ("bg", "surface-1", "surface-2")


def load_scale(theme):
    """Single-hue (turquoise) 7-step scale for load and other magnitudes; low -> high."""
    if theme == "dark":
        Ls = [0.30, 0.38, 0.46, 0.54, 0.62, 0.71, 0.80]
        Cs = [0.045, 0.06, 0.075, 0.09, 0.105, 0.115, 0.125]
    else:
        Ls = [0.93, 0.85, 0.77, 0.69, 0.61, 0.53, 0.45]
        Cs = [0.035, 0.05, 0.065, 0.08, 0.09, 0.095, 0.09]
    return [to_hex(oklch_to_srgb(L, C, 190)) for L, C in zip(Ls, Cs)]


HEX = {theme: {k: to_hex(oklch_to_srgb(*v)) for k, v in values.items()} for theme, values in THEMES.items()}


def rgba(hx, alpha):
    r, g, b = (int(hx[i:i + 2], 16) for i in (1, 3, 5))
    return f"rgba({r},{g},{b},{f'{alpha:.2f}'.lstrip('0')})"


def camel(name):
    head, *rest = name.split("-")
    return head + "".join(part[:1].upper() + part[1:] for part in rest)


# ---------------------------------------------------------------- contrast gate

def gate():
    failures = []
    for theme, c in HEX.items():
        print(f"== {theme}: contrast (text >= 4.5, graphics >= 3)")
        for surface in GATE_SURFACES:
            row = []
            for token in TEXT_TOKENS + GRAPHIC_TOKENS:
                ratio = contrast(c[token], c[surface])
                need = 3.0 if token in GRAPHIC_TOKENS else 4.5
                row.append(f"{token}={ratio:.2f}")
                if ratio < need:
                    failures.append(f"{theme}: {token} on {surface} = {ratio:.2f} (needs {need})")
            print(f"  {surface:9s} " + "  ".join(row))
        on_accent = c["bg"]   # primary buttons use --bg as their text color
        ratio = contrast(on_accent, c["turquoise"])
        print(f"  text {on_accent} on the turquoise button: {ratio:.2f}")
        if ratio < 4.5:
            failures.append(f"{theme}: button text on turquoise = {ratio:.2f} (needs 4.5)")
        ink, alpha = LINES[theme]["line"]
        line = blend(ink, c["surface-1"], alpha)
        print(f"  hairline over surface-1 = {line} ({contrast(line, c['surface-1']):.2f}:1, decorative)")
    return failures


# ---------------------------------------------------------------- outputs

def dtcg():
    tokens = {"$description": "BIRLIK design tokens (W3C DTCG format). Generated by design/colors.py — do not edit by hand.", "color": {}}
    for theme, c in HEX.items():
        group = {k: {"$type": "color", "$value": v} for k, v in c.items()}
        group["load"] = {str(i + 1): {"$type": "color", "$value": v} for i, v in enumerate(load_scale(theme))}
        group["chart"] = {k: {"$type": "color", "$value": v} for k, v in CHART[theme].items()}
        tokens["color"][theme] = group
    tokens["color"]["status"] = {k: {"$type": "color", "$value": v} for k, v in STATUS.items()}
    tokens["font"] = {
        "family": {
            "sans": {"$type": "fontFamily", "$value": ["Inter", "system-ui", "sans-serif"]},
            "mono": {"$type": "fontFamily", "$value": ["IBM Plex Mono", "ui-monospace", "monospace"]},
        },
        "size": {k: {"$type": "dimension", "$value": f"{v}px"} for k, v in FONT_SIZE.items()},
    }
    tokens["space"] = {str(v): {"$type": "dimension", "$value": f"{v}px"} for v in SPACE}
    tokens["radius"] = {k: {"$type": "dimension", "$value": f"{v}px"} for k, v in RADIUS.items()}
    tokens["motion"] = {k: {"$type": "duration", "$value": f"{v}ms"} for k, v in MOTION_MS.items()}
    tokens["motion"]["easing"] = {"$type": "cubicBezier", "$value": list(EASING)}
    return tokens


def css_block(theme, indent="  "):
    lines = [f"{indent}--{k}: {v};" for k, v in HEX[theme].items()]
    lines += [f"{indent}--load-{i + 1}: {v};" for i, v in enumerate(load_scale(theme))]
    lines += [f"{indent}--chart-{k}: {v};" for k, v in CHART[theme].items()]
    lines += [f"{indent}--{k}: {rgba(*v)};" for k, v in LINES[theme].items()]
    lines.append(f"{indent}color-scheme: {theme};")
    return "\n".join(lines)


def css():
    status = "\n".join(f"  --status-{k}: {v};" for k, v in STATUS.items())
    return f"""/* BIRLIK design tokens — generated by design/colors.py. Do not edit by hand. */
:root {{
{css_block('dark')}
{status}
}}
@media (prefers-color-scheme: light) {{
  :root:not([data-theme="dark"]) {{
{css_block('light', '    ')}
  }}
}}
:root[data-theme="light"] {{
{css_block('light')}
}}
:root[data-theme="dark"] {{
{css_block('dark')}
}}
"""


PALETTE_KEYS = list(THEMES["dark"].keys())


def swift():
    def color(hx, alpha=1.0):
        return f"Color(birlikHex: 0x{hx[1:].upper()}" + (f", opacity: {alpha}" if alpha != 1.0 else "") + ")"

    def palette(theme):
        args = [f"            {camel(k)}: {color(v)}" for k, v in HEX[theme].items()]
        args += [f"            {camel(k)}: {color(*v)}" for k, v in LINES[theme].items()]
        args += [f"            chart{camel(k)[:1].upper() + camel(k)[1:]}: {color(v)}" for k, v in CHART[theme].items()]
        args.append("            load: [" + ", ".join(color(v) for v in load_scale(theme)) + "]")
        return "BirlikPalette(\n" + ",\n".join(args) + "\n        )"

    fields = [f"    public let {camel(k)}: Color" for k in PALETTE_KEYS]
    fields += ["    public let line: Color", "    public let line2: Color"]
    fields += [f"    public let chart{k[:1].upper() + k[1:]}: Color" for k in CHART["dark"]]
    fields.append("    public let load: [Color]")
    sizes = "\n".join(f"        public static let {camel(k).replace('-', '')}: CGFloat = {v}" for k, v in FONT_SIZE.items())
    space = "\n".join(f"        public static let s{v}: CGFloat = {v}" for v in SPACE)
    radius = "\n".join(f"        public static let {k}: CGFloat = {v}" for k, v in RADIUS.items())
    motion = "\n".join(f"        public static let {k}: Double = {v / 1000}" for k, v in MOTION_MS.items())
    status = "\n".join(f"        public static let {k} = {color(v)}" for k, v in STATUS.items())
    return f"""// BIRLIK design tokens — generated by design/colors.py. Do not edit by hand.
import SwiftUI

public struct BirlikPalette: Sendable {{
{chr(10).join(fields)}
}}

public enum BirlikTokens {{
    public static let dark = {palette('dark')}

    public static let light = {palette('light')}

    public enum Status {{
{status}
    }}

    public enum FontSize {{
{sizes}
    }}

    public enum Space {{
{space}
    }}

    public enum Radius {{
{radius}
    }}

    /// Durations in seconds; easing is cubic-bezier{EASING}.
    public enum Motion {{
{motion}
        public static let easing: (Double, Double, Double, Double) = {EASING}
    }}
}}

extension Color {{
    init(birlikHex hex: UInt32, opacity: Double = 1) {{
        self.init(.sRGB,
                  red: Double((hex >> 16) & 0xFF) / 255,
                  green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255,
                  opacity: opacity)
    }}
}}
"""


def kotlin():
    def color(hx, alpha=1.0):
        a = round(alpha * 255)
        return f"Color(0x{a:02X}{hx[1:].upper()})"

    def palette(theme):
        args = [f"        {camel(k)} = {color(v)}" for k, v in HEX[theme].items()]
        args += [f"        {camel(k)} = {color(*v)}" for k, v in LINES[theme].items()]
        args += [f"        chart{k[:1].upper() + k[1:]} = {color(v)}" for k, v in CHART[theme].items()]
        args.append("        load = listOf(" + ", ".join(color(v) for v in load_scale(theme)) + ")")
        return "BirlikPalette(\n" + ",\n".join(args) + ",\n    )"

    fields = [f"    val {camel(k)}: Color," for k in PALETTE_KEYS]
    fields += ["    val line: Color,", "    val line2: Color,"]
    fields += [f"    val chart{k[:1].upper() + k[1:]}: Color," for k in CHART["dark"]]
    fields.append("    val load: List<Color>,")
    sizes = "\n".join(f"        val {camel(k)} = {v}.sp" for k, v in FONT_SIZE.items())
    space = "\n".join(f"        val s{v} = {v}.dp" for v in SPACE)
    radius = "\n".join(f"        val {k} = {v}.dp" for k, v in RADIUS.items())
    motion = "\n".join(f"        const val {k.upper()}_MS = {v}" for k, v in MOTION_MS.items())
    status = "\n".join(f"        val {k} = {color(v)}" for k, v in STATUS.items())
    return f"""// BIRLIK design tokens — generated by design/colors.py. Do not edit by hand.
package birlik.design

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

data class BirlikPalette(
{chr(10).join(fields)}
)

object BirlikTokens {{
    val dark = {palette('dark')}

    val light = {palette('light')}

    object Status {{
{status}
    }}

    object FontSize {{
{sizes}
    }}

    object Space {{
{space}
    }}

    object Radius {{
{radius}
    }}

    /** Durations in milliseconds; easing is cubic-bezier{EASING}. */
    object Motion {{
{motion}
    }}
}}
"""


def typescript():
    def palette(theme):
        entries = {camel(k): v for k, v in HEX[theme].items()}
        entries.update({camel(k): rgba(*v) for k, v in LINES[theme].items()})
        entries.update({f"chart{k[:1].upper() + k[1:]}": v for k, v in CHART[theme].items()})
        entries["load"] = load_scale(theme)
        return json.dumps(entries, indent=2).replace("\n", "\n  ")

    return f"""// BIRLIK design tokens — generated by design/colors.py. Do not edit by hand.
export const palette = {{
  dark: {palette('dark')},
  light: {palette('light')},
}} as const;

export const status = {json.dumps(STATUS, indent=2)} as const;
export const fontSize = {json.dumps({camel(k): v for k, v in FONT_SIZE.items()}, indent=2)} as const;
export const space = {json.dumps(list(SPACE))} as const;
export const radius = {json.dumps(RADIUS, indent=2)} as const;
export const motionMs = {json.dumps(MOTION_MS, indent=2)} as const;
export const easing = {json.dumps(list(EASING))} as const;

export type ThemeName = keyof typeof palette;
"""


def main():
    failures = gate()
    (HERE / "colors.json").write_text(json.dumps(HEX, indent=1) + "\n")
    (HERE / "tokens.json").write_text(json.dumps(dtcg(), indent=1, ensure_ascii=False) + "\n")
    (HERE / "tokens.css").write_text(css())
    platforms = HERE / "platforms"
    platforms.mkdir(exist_ok=True)
    (platforms / "BirlikTokens.swift").write_text(swift())
    (platforms / "BirlikTokens.kt").write_text(kotlin())
    (platforms / "tokens.ts").write_text(typescript())
    print("\nload dark :", load_scale("dark"))
    print("load light:", load_scale("light"))
    if failures:
        print("\nCONTRAST GATE FAILED:\n  " + "\n  ".join(failures), file=sys.stderr)
        sys.exit(1)
    print("\ncontrast gate: passed")


if __name__ == "__main__":
    main()
