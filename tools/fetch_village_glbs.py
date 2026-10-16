#!/usr/bin/env python3
"""빌리지 건물 GLB 페치·리컬러·병합 — Medieval Village/Farm Buildings (Quaternius CC0).

원천: poly.pizza 번들(Medieval-Village-Pack, Farm-Buildings-Bundle) → /m/{id} 페이지에서
static.poly.pizza/{uuid}.glb 추출(메모리 절차 — 단축 ID 직접은 403).

poly.pizza GLB는 플랫 컬러 세대(baseColorFactor가 어둡게 구움 — Wood #1F0D04 등).
재질명 기준으로 참조 팔레트(코지 페인티드 — 아이보리 벽·꿀색 목재·테라코타/청색 지붕)로
리컬러한다. 변형별 지붕색 오버라이드로 마을 다양성 확보.

산출:
  assets/models/CottageHouses.glb — House_1(테라코타)·House_2(청색)·House_4(주황) 병합
  assets/models/Barn.glb          — 헛간(StorageHut 대체)
용법: python3 tools/fetch_village_glbs.py [작업dir] — 이미 받아둔 glb가 있으면 재사용.
"""
import json
import struct
import sys
import urllib.request
from pathlib import Path

ASSETS = Path(__file__).resolve().parent.parent / "assets" / "models"

# poly.pizza 모델 ID → (산출명, 메시명, 지붕 재질 오버라이드)
COTTAGES = [
    ("341c9305-c6dd-4427-ad00-620ad57eb14e", "House_1", {"RoofTiles": "#B85C42"}),
    ("b8a2c719-5a70-4d6c-8049-90af7e3f89ee", "House_2", {"RoofTiles": "#4E7EA8"}),
    ("c08c3ac7-e65c-41f5-b200-b00c12103e57", "House_4", {"RoofTiles": "#D9813F", "RoofTiles_Red": "#D9813F"}),
]
BARN = ("e0ecaf1c-bff4-4f30-9a53-0cf0c1c90deb",)

# 재질명 → 참조 팔레트(편집 후 gen: 그대로 유지 — 원본 Quaternius 명명 준수)
RECOLOR = {
    "Plaster": "#F2E8D5",
    "Beige": "#E8DCC4",
    "Wood": "#6B4A2E",
    "Wood_Light": "#C89B6A",
    "Wood_Side": "#9C7450",
    "RoofTiles": "#B85C42",
    "RoofTiles_Red": "#C25B44",
    "Stone_Light": "#B0A89C",
    "Stone_Dark": "#8F877B",
    "Stone": "#6E685F",
    "Windows": "#2E4A66",
    "Green": "#5E8C3F",
    "DarkRed": "#A8422F",
    "LightRed": "#C25B44",
    "White": "#F2EFE6",
    "RoofBlack": "#4A4440",
    "Black": "#2A2A2C",
    "DarkGrey": "#5A5A5C",
    "Grey": "#8A8886",
    "LightBrown": "#B08A5C",
    "DarkBrown": "#5C4326",
}

UA = {"User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)"}


def fetch(uuid: str, workdir: Path) -> bytes:
    path = workdir / f"{uuid}.glb"
    if path.exists():
        return path.read_bytes()
    req = urllib.request.Request(f"https://static.poly.pizza/{uuid}.glb", headers=UA)
    data = urllib.request.urlopen(req, timeout=30).read()
    path.write_bytes(data)
    return data


def parse_glb(data: bytes):
    """GLB → (json, bin bytes)."""
    magic, ver, total = struct.unpack("<III", data[:12])
    assert magic == 0x46546C67, "not a glb"
    jlen, jtype = struct.unpack("<II", data[12:20])
    assert jtype == 0x4E4F534A
    j = json.loads(data[20 : 20 + jlen])
    off = 20 + jlen
    binb = b""
    if off < total:
        blen, btype = struct.unpack("<II", data[off : off + 8])
        assert btype == 0x004E4942
        binb = data[off + 8 : off + 8 + blen]
    return j, binb


def hex2rgb(h):
    return [int(h[i : i + 2], 16) / 255 for i in (1, 3, 5)] + [1.0]


def recolor(j: dict, roof_override: dict):
    """재질명 기준 baseColorFactor 재지정."""
    for m in j.get("materials", []):
        name = m.get("name", "")
        target = roof_override.get(name) or RECOLOR.get(name)
        if target:
            m.setdefault("pbrMetallicRoughness", {})["baseColorFactor"] = hex2rgb(target)
            m["pbrMetallicRoughness"]["metallicFactor"] = 0.0
            m["pbrMetallicRoughness"]["roughnessFactor"] = 0.9
    return j


def merge(sources):
    """[(json, bin, mesh_name, node_name)] → 단일 (json, bin) — 인덱스 오프셋 병합."""
    out_j = {
        "asset": {"version": "2.0", "generator": "terrafolk fetch_village_glbs"},
        "scene": 0,
        "scenes": [{"nodes": []}],
        "nodes": [],
        "meshes": [],
        "materials": [],
        "accessors": [],
        "bufferViews": [],
        "buffers": [],
    }
    out_bin = bytearray()
    for j, binb, mesh_name, node_name in sources:
        # 재지정 전 인덱스 매핑
        n_acc = len(out_j["accessors"])
        n_bv = len(out_j["bufferViews"])
        n_mat = len(out_j["materials"])
        # 버퍼 — 단일 BIN에 이어 붙임(4바이트 정렬)
        pad = (-len(out_bin)) % 4
        out_bin += b"\x00" * pad
        base = len(out_bin)
        out_bin += binb
        for bv in j.get("bufferViews", []):
            bv = dict(bv)
            bv["buffer"] = 0
            bv["byteOffset"] = bv.get("byteOffset", 0) + base
            out_j["bufferViews"].append(bv)
        for acc in j.get("accessors", []):
            acc = dict(acc)
            if "bufferView" in acc:
                acc["bufferView"] += n_bv
            out_j["accessors"].append(acc)
        for mat in j.get("materials", []):
            out_j["materials"].append(mat)
        for mesh in j.get("meshes", []):
            mesh = json.loads(json.dumps(mesh))
            for prim in mesh.get("primitives", []):
                for key in ("indices",):
                    if key in prim:
                        prim[key] += n_acc
                for key in ("POSITION", "NORMAL", "TANGENT", "TEXCOORD_0", "TEXCOORD_1", "COLOR_0"):
                    if key in prim.get("attributes", {}):
                        prim["attributes"][key] += n_acc
                if "material" in prim:
                    prim["material"] += n_mat
            mesh["name"] = mesh_name
            out_j["meshes"].append(mesh)
            out_j["nodes"].append({"mesh": len(out_j["meshes"]) - 1, "name": node_name})
            out_j["scenes"][0]["nodes"].append(len(out_j["nodes"]) - 1)
    out_j["buffers"].append({"byteLength": len(out_bin)})
    return out_j, bytes(out_bin)


def write_glb(j: dict, binb: bytes, path: Path):
    jbytes = json.dumps(j).encode()
    jbytes += b" " * ((-len(jbytes)) % 4)
    binb = binb + b"\x00" * ((-len(binb)) % 4)
    total = 12 + 8 + len(jbytes) + 8 + len(binb)
    head = struct.pack("<III", 0x46546C67, 2, total)
    jc = struct.pack("<II", len(jbytes), 0x4E4F534A)
    bc = struct.pack("<II", len(binb), 0x004E4942)
    path.write_bytes(head + jc + jbytes + bc + binb)
    print(f"wrote {path.name} {total // 1024}KB meshes={[m['name'] for m in j['meshes']]}")


def main():
    workdir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/tmp/tf-shot/glbs")
    workdir.mkdir(parents=True, exist_ok=True)

    # 코티지 3종 병합(변형 지붕색)
    srcs = []
    for uuid, mesh_name, roof in COTTAGES:
        j, binb = parse_glb(fetch(uuid, workdir))
        j = recolor(j, roof)
        srcs.append((j, binb, mesh_name, mesh_name))
    j, binb = merge(srcs)
    write_glb(j, binb, ASSETS / "CottageHouses.glb")

    # 헛간
    j, binb = parse_glb(fetch(BARN[0], workdir))
    write_glb(recolor(j, {}), binb, ASSETS / "Barn.glb")


if __name__ == "__main__":
    main()
