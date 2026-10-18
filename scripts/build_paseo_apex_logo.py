import os
import subprocess
from PIL import Image

def build_logos():
    os.makedirs('assets/branding', exist_ok=True)
    os.makedirs('packages/desktop-tauri/src-tauri/icons', exist_ok=True)

    vanguard_svg = '''<svg width="512" height="512" viewBox="0 0 512 512" fill="none" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <!-- Background Chassis Gradient -->
    <radialGradient id="chassisBg" cx="50%" cy="38%" r="65%">
      <stop offset="0%" stop-color="#1b2029"/>
      <stop offset="50%" stop-color="#0f131a"/>
      <stop offset="85%" stop-color="#080a0e"/>
      <stop offset="100%" stop-color="#040507"/>
    </radialGradient>

    <!-- Outer Armor Bevel Rim -->
    <linearGradient id="armorBevel" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#475569"/>
      <stop offset="25%" stop-color="#1e293b"/>
      <stop offset="70%" stop-color="#0f172a"/>
      <stop offset="100%" stop-color="#334155"/>
    </linearGradient>

    <!-- Metallic Titanium Light Facets (Specular Bright) -->
    <linearGradient id="titaniumLight" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#ffffff"/>
      <stop offset="20%" stop-color="#f1f5f9"/>
      <stop offset="55%" stop-color="#cbd5e1"/>
      <stop offset="85%" stop-color="#94a3b8"/>
      <stop offset="100%" stop-color="#64748b"/>
    </linearGradient>

    <!-- Metallic Titanium Midtone Facets -->
    <linearGradient id="titaniumMid" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#cbd5e1"/>
      <stop offset="40%" stop-color="#94a3b8"/>
      <stop offset="80%" stop-color="#64748b"/>
      <stop offset="100%" stop-color="#475569"/>
    </linearGradient>

    <!-- Metallic Gunmetal Shadow Facets (Deep Shadow) -->
    <linearGradient id="titaniumDark" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#64748b"/>
      <stop offset="35%" stop-color="#475569"/>
      <stop offset="75%" stop-color="#1e293b"/>
      <stop offset="100%" stop-color="#0f172a"/>
    </linearGradient>

    <!-- Electric Cyan Neon Gradient -->
    <linearGradient id="cyanNeon" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#00f5ff" stop-opacity="0.2"/>
      <stop offset="15%" stop-color="#00f5ff" stop-opacity="0.95"/>
      <stop offset="50%" stop-color="#00e5ff" stop-opacity="1"/>
      <stop offset="85%" stop-color="#00b4d8" stop-opacity="0.95"/>
      <stop offset="100%" stop-color="#0077b6" stop-opacity="0.2"/>
    </linearGradient>

    <!-- Neon Glow Filter -->
    <filter id="cyanGlow" x="-80%" y="-40%" width="260%" height="180%">
      <feGaussianBlur stdDeviation="7" result="blur1"/>
      <feGaussianBlur stdDeviation="15" result="blur2"/>
      <feMerge>
        <feMergeNode in="blur2"/>
        <feMergeNode in="blur1"/>
        <feMergeNode in="SourceGraphic"/>
      </feMerge>
    </filter>

    <!-- Center Agent Glow Filter -->
    <filter id="coreGlow" x="-50%" y="-50%" width="200%" height="200%">
      <feGaussianBlur stdDeviation="8" result="cblur"/>
      <feMerge>
        <feMergeNode in="cblur"/>
        <feMergeNode in="SourceGraphic"/>
      </feMerge>
    </filter>
  </defs>

  <!-- 1. Stealth Armor Chassis Plate -->
  <rect x="20" y="20" width="472" height="472" rx="98" fill="url(#chassisBg)" stroke="url(#armorBevel)" stroke-width="3.5"/>

  <!-- Subtle Inset Stealth Grooves -->
  <path d="M 120 32 L 392 32 M 32 120 L 32 392 M 480 120 L 480 392 M 120 480 L 392 480" stroke="#1e2633" stroke-width="1.5" stroke-opacity="0.6"/>

  <!-- 2. Electric Cyan Curved Neon Lightbars (Left & Right) -->
  <!-- Left Neon Lightbar -->
  <g filter="url(#cyanGlow)">
    <path d="M 28 80 C 18 160, 24 320, 56 424 C 62 424, 66 418, 64 406 C 36 312, 32 170, 42 90 Z" fill="url(#cyanNeon)"/>
  </g>
  <!-- Left Neon Core Highlight Strip -->
  <path d="M 33 110 C 26 175, 30 300, 54 394 C 55 394, 56 390, 56 384 C 36 295, 32 180, 39 116 Z" fill="#ffffff" opacity="0.9"/>

  <!-- Right Neon Lightbar (Symmetric) -->
  <g filter="url(#cyanGlow)">
    <path d="M 484 80 C 494 160, 488 320, 456 424 C 450 424, 446 418, 448 406 C 476 312, 480 170, 470 90 Z" fill="url(#cyanNeon)"/>
  </g>
  <!-- Right Neon Core Highlight Strip -->
  <path d="M 479 110 C 486 175, 482 300, 458 394 C 457 394, 456 390, 456 384 C 476 295, 480 180, 473 116 Z" fill="#ffffff" opacity="0.9"/>

  <!-- 3. The Paseo Apex Mechanical Predator Crest -->
  <g id="apexCrest">
    <!-- ==================== LEFT WING ==================== -->
    <!-- Left Wing Outer Tall Horn (Main Light Surface) -->
    <polygon points="104,80 148,156 142,270 102,236" fill="url(#titaniumLight)"/>
    <!-- Left Wing Outer Tall Horn (Chamfered Bevel Edge) -->
    <polygon points="104,80 148,156 138,158 108,86" fill="url(#titaniumDark)"/>

    <!-- Left Wing Center Triangular Step & Notch Body -->
    <polygon points="148,156 216,212 180,240 142,270" fill="url(#titaniumMid)"/>
    <polygon points="148,156 166,224 142,270" fill="url(#titaniumDark)" opacity="0.7"/>

    <!-- Left Wing Inner Vertical Spine / Blade -->
    <polygon points="244,116 244,360 214,354 212,216" fill="url(#titaniumLight)"/>
    <!-- Left Wing Inner Vertical Spine (Bevel Shadow) -->
    <polygon points="244,116 244,360 236,360 236,136" fill="url(#titaniumDark)"/>

    <!-- Left Wing Lower Downward Talon / Fang -->
    <polygon points="180,360 244,360 182,438 180,390" fill="url(#titaniumLight)"/>
    <!-- Left Wing Lower Downward Talon (Shadow Bevel) -->
    <polygon points="214,354 244,360 182,438 206,370" fill="url(#titaniumDark)"/>

    <!-- ==================== RIGHT WING (Symmetrical Mirror) ==================== -->
    <!-- Right Wing Outer Tall Horn (Main Light Surface) -->
    <polygon points="408,80 364,156 370,270 410,236" fill="url(#titaniumMid)"/>
    <!-- Right Wing Outer Tall Horn (Chamfered Bevel Edge) -->
    <polygon points="408,80 364,156 374,158 404,86" fill="url(#titaniumDark)"/>

    <!-- Right Wing Center Triangular Step & Notch Body -->
    <polygon points="364,156 296,212 332,240 370,270" fill="url(#titaniumDark)"/>
    <polygon points="364,156 346,224 370,270" fill="#0f172a"/>

    <!-- Right Wing Inner Vertical Spine / Blade -->
    <polygon points="268,116 268,360 298,354 300,216" fill="url(#titaniumMid)"/>
    <!-- Right Wing Inner Vertical Spine (Bevel Shadow) -->
    <polygon points="268,116 268,360 276,360 276,136" fill="url(#titaniumDark)"/>

    <!-- Right Wing Lower Downward Talon / Fang -->
    <polygon points="332,360 268,360 330,438 332,390" fill="url(#titaniumMid)"/>
    <!-- Right Wing Lower Downward Talon (Shadow Bevel) -->
    <polygon points="298,354 268,360 330,438 306,370" fill="url(#titaniumDark)"/>

    <!-- ==================== PASEO QUANTUM AGENT CORE ==================== -->
    <!-- Center glowing diamond nexus positioned between the inner blades -->
    <polygon points="256,220 266,256 256,292 246,256" fill="#00f0ff" filter="url(#coreGlow)"/>
    <polygon points="256,230 262,256 256,282 250,256" fill="#ffffff"/>
  </g>

  <!-- 4. Windows 11 Running App Indicator Pill / Dock Anchor -->
  <rect x="228" y="456" width="56" height="8" rx="4" fill="#475569" opacity="0.65"/>
  <rect x="238" y="458" width="36" height="4" rx="2" fill="#00f0ff" opacity="0.9" filter="url(#cyanGlow)"/>
  <rect x="246" y="459" width="20" height="2" rx="1" fill="#ffffff" opacity="0.9"/>
</svg>'''

    vanguard_path = 'assets/branding/paseo-apex-vanguard.svg'
    with open(vanguard_path, 'w', encoding='utf-8') as f:
        f.write(vanguard_svg)
    print(f"Wrote {vanguard_path}")

    # Render Vanguard PNG at 512x512 with Inkscape
    vanguard_png = 'assets/branding/paseo-apex-vanguard.png'
    subprocess.run(['inkscape', '--export-filename=' + vanguard_png, '-w', '512', '-h', '512', vanguard_path], check=True)
    print(f"Exported {vanguard_png}")

    # Generate multi-size ICO file for Windows desktop app
    im = Image.open(vanguard_png)
    ico_path = 'packages/desktop-tauri/src-tauri/icons/icon.ico'
    im.save(ico_path, format='ICO', sizes=[(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)])
    print(f"Generated multi-res {ico_path}")

    # Also save 512x512 icon.png for Tauri app
    icon_png_path = 'packages/desktop-tauri/src-tauri/icons/icon.png'
    im.save(icon_png_path, format='PNG')
    print(f"Saved {icon_png_path}")

    # Copy to assets/branding/icon.ico as well
    im.save('assets/branding/icon.ico', format='ICO', sizes=[(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)])

    # Also copy SVG to public website logo for previewing
    with open('packages/website/public/logo-cyber.svg', 'w', encoding='utf-8') as f:
        f.write(vanguard_svg)
    print("Exported packages/website/public/logo-cyber.svg")

if __name__ == '__main__':
    build_logos()
