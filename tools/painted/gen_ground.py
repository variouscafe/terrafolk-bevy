#!/usr/bin/env python3
"""페인티드 지형 텍스처 5종 생성 — Quaternius stylized 팔레트 정합.
타일랩(4에지 랩)·고해상도(512)·부드러운 붓터치. 픽셀아트 아님(선형 필터 전제).

팔레트 원천: Trees_Normal.glb 텍스처 아이드롭
  bark #955D46 · leaves #528700/#7CCD00 — 지형은 이 녹색/갈색 계열로 파생.
"""
import math
import random
from PIL import Image, ImageDraw, ImageFilter

SIZE = 512

def wrap_value_noise(seed, period_cells=4, octaves=(1.0, 0.5, 0.25), base=0.0, amp=1.0):
    """랩 코사인 보간 값노이즈 — 타일 시임 없음. 이방성 스케일 지원."""
    grids = []
    for oi, w in enumerate(octaves):
        # 옥타브 i: 셀 수 = period_cells * 2^i (w는 가중치)
        n = max(2, period_cells * (2 ** oi))
        rng = random.Random(seed * 977 + oi * 131)
        grids.append((n, w, [[rng.uniform(-1, 1) for _ in range(n)] for _ in range(n)]))
    def sample(u, v, sx=1.0, sy=1.0):
        total, tw = 0.0, 0.0
        for n, w, g in grids:
            x, y = u * sx * n, v * sy * n
            x0, y0 = int(x) % n, int(y) % n
            x1, y1 = (x0 + 1) % n, (y0 + 1) % n
            fx, fy = x - int(x), y - int(y)
            cx = (1 - math.cos(fx * math.pi)) / 2
            cy = (1 - math.cos(fy * math.pi)) / 2
            a, b = g[y0][x0], g[y0][x1]
            c, d = g[y1][x0], g[y1][x1]
            total += (a + (b - a) * cx + (c + (d - c) * cx - a - (b - a) * cx) * cy) * w
            tw += w
        return base + (total / tw) * amp  # 대략 [base-amp, base+amp]
    return sample

def lerp(a, b, t): return a + (b - a) * t

def hex2rgb(h): return tuple(int(h[i:i+2], 16) for i in (1, 3, 5))

def mix(c1, c2, t): return tuple(max(0, min(255, round(lerp(c1[i], c2[i], t)))) for i in range(3))

def jitter_hue(c, rng, deg=6, sat_amp=0.06, val_amp=0.08):
    """HSV 미세 지터 — 붓터치 색 변화."""
    import colorsys
    h, s, v = colorsys.rgb_to_hsv(*[x / 255 for x in c])
    h = (h + rng.uniform(-deg / 360, deg / 360)) % 1.0
    s = max(0, min(1, s * (1 + rng.uniform(-sat_amp, sat_amp))))
    v = max(0, min(1, v * (1 + rng.uniform(-val_amp, val_amp))))
    return tuple(round(x * 255) for x in colorsys.hsv_to_rgb(h, s, v))

def paint_base(size, rng_seed, colors, weights, noise, blob_scale=3.0, sx=1.0, sy=1.0):
    """노이즈→팔레트 다중 보간 베이스."""
    rng = random.Random(rng_seed * 7 + 3)
    im = Image.new('RGB', (size, size))
    px = im.load()
    n_col = len(colors) - 1
    for y in range(size):
        for x in range(size):
            t = noise(x / size, y / size, sx, sy) * 0.5 + 0.5
            t = max(0.0, min(1.0, t))
            f = t * n_col
            i = min(int(f), n_col - 1)
            c = mix(colors[i], colors[i + 1], f - i)
            px[x, y] = jitter_hue(c, rng, deg=5)
    # 큰 붓 블롭 오버레이(팔레트 내 왕복)
    overlay = Image.new('RGB', (size, size))
    dr = ImageDraw.Draw(overlay)
    for _ in range(26):
        r = rng.randint(28, 90)
        x, y = rng.randint(-r, size + r), rng.randint(-r, size + r)
        c = colors[rng.randint(0, len(colors) - 1)]
        dr.ellipse([x - r, y - r, x + r, y + r], fill=jitter_hue(c, rng, deg=7))
    overlay = overlay.filter(ImageFilter.GaussianBlur(18))
    im = Image.blend(im, overlay, 0.30)
    return im

def dab_strokes(im, rng_seed, color_fns, n, len_rng, width, ang_center, ang_spread, alpha_layer=None):
    """붓터치 스트로크 — 짧은 방향성 대시. color_fns: (t)->rgb."""
    rng = random.Random(rng_seed * 31 + 11)
    layer = Image.new('RGBA', (SIZE, SIZE), (0, 0, 0, 0))
    dr = ImageDraw.Draw(layer)
    for _ in range(n):
        x, y = rng.randint(0, SIZE - 1), rng.randint(0, SIZE - 1)
        ln = rng.randint(*len_rng)
        ang = math.radians(ang_center + rng.uniform(-ang_spread, ang_spread))
        dx, dy = math.cos(ang) * ln, math.sin(ang) * ln
        wdt = rng.randint(max(1, width - 1), width + 1)
        t = rng.random()
        c = color_fns[0](t) if isinstance(color_fns, list) else color_fns(t)
        a = rng.randint(110, 190)
        # 랩: 4방향 오프셋으로 그려 시임 방지
        for ox in (-SIZE, 0, SIZE):
            for oy in (-SIZE, 0, SIZE):
                dr.line([x + ox, y + oy, x + ox + dx, y + oy + dy], fill=c + (a,), width=wdt)
    layer = layer.filter(ImageFilter.GaussianBlur(0.6))
    base = im.convert('RGBA')
    base.alpha_composite(layer)
    return base.convert('RGB')

def speckles(im, rng_seed, color_fns, n, rmin, rmax, blur=0.4):
    rng = random.Random(rng_seed * 53 + 5)
    layer = Image.new('RGBA', (SIZE, SIZE), (0, 0, 0, 0))
    dr = ImageDraw.Draw(layer)
    for _ in range(n):
        x, y = rng.randint(0, SIZE - 1), rng.randint(0, SIZE - 1)
        r = rng.randint(rmin, rmax)
        c = color_fns[0](rng.random()) if isinstance(color_fns, list) else color_fns(rng.random())
        a = rng.randint(130, 210)
        for ox in (-SIZE, 0, SIZE):
            for oy in (-SIZE, 0, SIZE):
                dr.ellipse([x - r + ox, y - r + oy, x + r + ox, y + r + oy], fill=c + (a,))
    if blur: layer = layer.filter(ImageFilter.GaussianBlur(blur))
    base = im.convert('RGBA'); base.alpha_composite(layer)
    return base.convert('RGB')

def finalize(im, name, out_dir):
    im = im.filter(ImageFilter.GaussianBlur(0.5))  # 최종 미소프트 — 페인티드
    # 2×2 타일 시임 검증용도 아니고 저장
    im.save(f"{out_dir}/{name}.png")
    return im

# ── 레이어 레시피 ──────────────────────────────────────────────
def gen_grass(out):
    # 잎새 그린 램프: 라임 그린→밝은 옐로그린(참조 #90A030~#B0B040)
    colors = [hex2rgb('#7FA83E'), hex2rgb('#93BC4A'), hex2rgb('#A8D058'), hex2rgb('#BCE468')]
    noise = wrap_value_noise(101, 5, (1.0, 0.45, 0.22))
    im = paint_base(SIZE, 101, colors, None, noise, sx=1.0, sy=1.0)
    # 세로 잎 스트로크(위에서 본 풀판 느낌의 짧은 대시, 방향 약간 흐트림)
    im = dab_strokes(im, 7, [lambda t: mix(hex2rgb('#8BB448'), hex2rgb('#B2D862'), t)], 900, (7, 16), 2, 78, 38)
    im = dab_strokes(im, 8, [lambda t: mix(hex2rgb('#71963A'), hex2rgb('#93BC4C'), t)], 420, (6, 13), 2, 80, 40)
    im = speckles(im, 9, [lambda t: mix(hex2rgb('#D2EC90'), hex2rgb('#E8F8B4'), t)], 150, 1, 2, 0.3)
    return finalize(im, 'grass', out)

def gen_forest(out):
    # 숲바닥 — 밝은 중간록 + 낙엽 litter(참조 숲 그린 #607030)
    colors = [hex2rgb('#4E7A2E'), hex2rgb('#5C8C38'), hex2rgb('#6CA044'), hex2rgb('#7EB654')]
    noise = wrap_value_noise(202, 4, (1.0, 0.4))
    im = paint_base(SIZE, 202, colors, None, noise)
    # 낙엽 대시 — 갈색/올리브 angled
    im = dab_strokes(im, 21, [lambda t: mix(hex2rgb('#7A5A34'), hex2rgb('#9C7644'), t)], 520, (8, 18), 3, 30, 60)
    im = dab_strokes(im, 22, [lambda t: mix(hex2rgb('#5E7632'), hex2rgb('#82A050'), t)], 380, (9, 20), 3, 55, 70)
    im = speckles(im, 23, [lambda t: mix(hex2rgb('#3A5A24'), hex2rgb('#4E7028'), t)], 260, 1, 3, 0.4)
    return finalize(im, 'forest', out)

def gen_sand(out):
    colors = [hex2rgb('#D2B98A'), hex2rgb('#E0C99C'), hex2rgb('#EAD7AC'), hex2rgb('#F4E5BE')]
    noise = wrap_value_noise(303, 4, (1.0, 0.5, 0.25))
    im = paint_base(SIZE, 303, colors, None, noise, sx=1.0, sy=0.45)  # 세로 늘어진 램프=물결
    # 잔물결 띠 — 왜곡 사인
    rng = random.Random(31)
    dr = ImageDraw.Draw(im, 'RGBA')
    for _ in range(26):
        y0 = rng.randint(0, SIZE)
        amp = rng.randint(6, 16)
        ph = rng.uniform(0, 6.28)
        c = mix(hex2rgb('#EED9A8'), hex2rgb('#F6E6BC'), rng.random())
        pts = []
        for x in range(0, SIZE + 8, 8):
            y = y0 + math.sin(x / SIZE * 2 * math.pi * rng.uniform(1.2, 2.2) + ph) * amp
            pts.append((x, y))
        for ox in (-SIZE, 0, SIZE):
            dr.line([(px + ox, py) for px, py in pts], fill=c + (70,), width=3)
    im = im.filter(ImageFilter.GaussianBlur(0.6))
    im = speckles(im, 33, [lambda t: mix(hex2rgb('#B79A66'), hex2rgb('#D8BC88'), t)], 220, 1, 2, 0.3)
    im = speckles(im, 34, [lambda t: mix(hex2rgb('#F2E3B8'), hex2rgb('#FAF0CC'), t)], 90, 1, 2, 0.3)
    return finalize(im, 'sand', out)

def gen_rock(out):
    # 따뜻한 회색 절벽(참조 #A8A096)
    colors = [hex2rgb('#8A857C'), hex2rgb('#9A948A'), hex2rgb('#A9A397'), hex2rgb('#B9B3A6')]
    noise = wrap_value_noise(404, 3, (1.0, 0.5))
    im = paint_base(SIZE, 404, colors, None, noise)
    # 균열 — 랜덤 워크 라인(어두운)
    rng = random.Random(41)
    dr = ImageDraw.Draw(im, 'RGBA')
    for _ in range(16):
        x, y = rng.randint(0, SIZE), rng.randint(0, SIZE)
        pts = [(x, y)]
        for _ in range(rng.randint(6, 14)):
            ang = rng.uniform(0, 6.28)
            x += math.cos(ang) * rng.randint(10, 26)
            y += math.sin(ang) * rng.randint(6, 16)
            pts.append((x, y))
        w = rng.randint(2, 3)
        for ox in (-SIZE, 0, SIZE):
            for oy in (-SIZE, 0, SIZE):
                dr.line([(px + ox, py + oy) for px, py in pts], fill=(96, 90, 82, 150), width=w)
    im = im.filter(ImageFilter.GaussianBlur(0.7))
    # 이끼 얼룩(녹색 미세)
    im = speckles(im, 42, [lambda t: mix(hex2rgb('#7E9455'), hex2rgb('#93A868'), t)], 130, 2, 5, 2.2)
    # 하이라이트
    im = speckles(im, 43, [lambda t: mix(hex2rgb('#C6C0B2'), hex2rgb('#D8D3C6'), t)], 110, 2, 6, 2.4)
    return finalize(im, 'rock', out)

def gen_dirt(out):
    # 크림베이지 흙길(참조 패스 #C9A876) — 밝은 황토
    colors = [hex2rgb('#A57C50'), hex2rgb('#B98F62'), hex2rgb('#CBA878'), hex2rgb('#D8BA8C')]
    noise = wrap_value_noise(505, 4, (1.0, 0.5, 0.25))
    im = paint_base(SIZE, 505, colors, None, noise)
    # 흙 덩이 블롭
    im = speckles(im, 51, [lambda t: mix(hex2rgb('#8A6B47'), hex2rgb('#A3835C'), t)], 200, 3, 8, 2.0)
    im = speckles(im, 52, [lambda t: mix(hex2rgb('#C7A87D'), hex2rgb('#D6B98E'), t)], 160, 2, 6, 1.8)
    # 작은 돌
    im = speckles(im, 53, [lambda t: mix(hex2rgb('#A5A096'), hex2rgb('#B8B4A9'), t)], 110, 2, 4, 0.5)
    # 근경 스트로크
    im = dab_strokes(im, 54, [lambda t: mix(hex2rgb('#A5835C'), hex2rgb('#BD9B70'), t)], 340, (8, 18), 2, 20, 80)
    return finalize(im, 'dirt', out)

if __name__ == '__main__':
    import sys, os
    out = sys.argv[1] if len(sys.argv) > 1 else 'assets/textures/ground'
    os.makedirs(out, exist_ok=True)
    gens = [gen_grass, gen_forest, gen_sand, gen_rock, gen_dirt]
    imgs = [g(out) for g in gens]
    # 검수용 몽타주
    W = SIZE * len(imgs) + (len(imgs) - 1) * 8
    sheet = Image.new('RGB', (W, SIZE + 20), (30, 30, 34))
    from PIL import ImageDraw
    d = ImageDraw.Draw(sheet)
    for i, imx in enumerate(imgs):
        sheet.paste(imx, (i * (SIZE + 8), 0))
        d.text((i * (SIZE + 8) + 6, SIZE + 4), gens[i].__name__.replace('gen_', ''), fill=(240, 240, 240))
    sheet.save('/tmp/ground_sheet.png')
    print('saved to', out, '— sheet /tmp/ground_sheet.png')
