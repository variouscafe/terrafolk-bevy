#!/usr/bin/env python3
"""GLB 단위 정규화 — 노드 균일 스케일을 지오메트리에 베이크한다.

원천: Quaternius 리깅 캐릭터·코티지 번들은 Blender 내보내기 특유의
「cm 저작 + 노드 ×100 스케일」 구조다. 정점(POSITION)은 cm 단위이고
 Armature/메시 노드에 scale=100이 붙어 시각적 크기를 메터로 맞춘다.
glTF 표준 구현(three.js 등)은 관절 글로벌 행렬에 ×100이 흡수되지만,
Bevy 스키닝은 이 구조에서 메시를 원시 cm 크기(~5mm)로 붕괴시킨다 —
주민이 화면에 보이지 않는 원인.

해결: 노드의 균일 스케일 s를 제거하고 수학적으로 동치인 변환을 데이터에 베이크:
  - 노드 scale → [1,1,1]
  - 서브트리 모든 자손 노드 translation × s  (월드 위치 보존)
  - 서브트리 메시 POSITION × s (accessor min/max 동기 갱신)
  - 해당 메시가 쓰는 skin의 inverseBindMatrix: x/y/z 열 ÷ s
    (관절 글로벌은 그대로, 정점은 ×s → IBM' = IBM·S⁻¹ 로 동치)
  - 서브트리 노드를 타깃하는 애니메이션 translation 채널 출력 × s
    (관절 로컬 이동값이 스케일 공간에 있으므로)

노말·조인트인덱스·가중치는 균일 스케일이라 불변. 중첩 스케일 노드는
바깥쪽부터 순차 처리(각 단계가 월드 위치를 보존하므로 임의 중첩에 동치).
스케일 노드가 없으면 무변화(멱등) — 전 asset을 안심하고 돌릴 수 있다.

사용: python3 tools/normalize_glb_units.py assets/models/VillagerMan.glb ...
     python3 tools/normalize_glb_units.py --uniform 100 assets/models/X.glb
--uniform S: 스케일 노드 없이 cm 단위로 저작된 파일(코티지 번들 등)에
전체 POSITION·노드 translation을 일괄 ×S 한다(월드 전체 확대 — skin 없는
정적 메시에만 쓸 것. skin이 있으면 IBM 정합이 깨지므로 스케일 노드 경로만 유효).
표준 라이브러리만 사용(struct+json). BIN 청크 길이 불변 — 제자리 재작성.
"""

import json
import struct
import sys

JSON_CHUNK = 0x4E4F534A  # "JSON"
BIN_CHUNK = 0x004E4942  # "BIN"


def read_glb(path):
    with open(path, "rb") as f:
        data = f.read()
    magic, _ver, _len = struct.unpack("<III", data[:12])
    if magic != 0x46546C67:
        raise SystemExit(f"{path}: GLB가 아님")
    off, json_bytes, bin_bytes = 12, None, None
    while off < len(data):
        clen, ctype = struct.unpack("<II", data[off : off + 8])
        chunk = data[off + 8 : off + 8 + clen]
        if ctype == JSON_CHUNK:
            json_bytes = chunk
        elif ctype == BIN_CHUNK:
            bin_bytes = chunk
        off += 8 + clen
    gltf = json.loads(json_bytes.rstrip(b"\x00 ").decode("utf-8"))
    # sparse accessor 미지원 — 베이크가 들판(sparse 저장 분량)을 놓쳐 조용히
    # 지오메트리를 망가뜨린다. 만나면 즉시 실패가 안전하다.
    for i, acc in enumerate(gltf.get("accessors", [])):
        if "sparse" in acc:
            raise SystemExit(f"{path}: accessor[{i}]가 sparse — 미지원, 변환 거부")
    return gltf, bytearray(bin_bytes)


def write_glb(path, gltf, bin_bytes):
    json_bytes = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_bytes += b" " * ((4 - len(json_bytes) % 4) % 4)  # 4바이트 정렬 패딩
    if len(bin_bytes) % 4:
        bin_bytes = bin_bytes + b"\x00" * (4 - len(bin_bytes) % 4)
    total = 12 + 8 + len(json_bytes) + 8 + len(bin_bytes)
    out = struct.pack("<III", 0x46546C67, 2, total)
    out += struct.pack("<II", len(json_bytes), JSON_CHUNK) + json_bytes
    out += struct.pack("<II", len(bin_bytes), BIN_CHUNK) + bytes(bin_bytes)
    with open(path, "wb") as f:
        f.write(out)


def accessor_span(gltf, acc_idx):
    """accessor → (buffer_view, byte_offset, count, comp_type, n_comp)."""
    acc = gltf["accessors"][acc_idx]
    bv = gltf["bufferViews"][acc["bufferView"]]
    base = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    n_comp = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[acc["type"]]
    comp_size = {5126: 4, 5123: 2, 5121: 1, 5125: 8}[acc["componentType"]]
    stride = bv.get("byteStride") or n_comp * comp_size
    return base, stride, acc["count"], comp_size, n_comp


def scale_positions(gltf, bin_bytes, mesh_idx, s, seen):
    prim = gltf["meshes"][mesh_idx]["primitives"]
    for p in prim:
        pos = p["attributes"].get("POSITION")
        if pos is None or pos in seen:
            continue
        seen.add(pos)
        base, stride, count, comp_size, n_comp = accessor_span(gltf, pos)
        assert comp_size == 4, "POSITION이 float32가 아님"
        for i in range(count):
            o = base + i * stride
            x, y, z = struct.unpack_from("<fff", bin_bytes, o)
            struct.pack_into("<fff", bin_bytes, o, x * s, y * s, z * s)
        acc = gltf["accessors"][pos]
        if "min" in acc:
            acc["min"] = [v * s for v in acc["min"]]
        if "max" in acc:
            acc["max"] = [v * s for v in acc["max"]]


def scale_inverse_bindposes(gltf, bin_bytes, skin_idx, s, seen):
    """IBM' = IBM·S⁻¹ — x/y/z 기저열(0,1,2)만 ÷s. 열 우선(mat4) 저장."""
    acc_idx = gltf["skins"][skin_idx]["inverseBindMatrices"]
    if acc_idx in seen:
        return
    seen.add(acc_idx)
    base, stride, count, comp_size, n_comp = accessor_span(gltf, acc_idx)
    assert comp_size == 4 and n_comp == 16, "IBM이 float32 mat4가 아님"
    for i in range(count):
        o = base + i * stride
        m = struct.unpack_from("<16f", bin_bytes, o)
        m = list(m)
        for col in (0, 1, 2):
            for row in range(4):
                m[col * 4 + row] /= s
        struct.pack_into("<16f", bin_bytes, o, *m)


def scale_anim_translations_indexed(gltf, bin_bytes, node_ids, s, seen):
    """node_ids 중 하나를 타깃하는 translation 채널의 출력값 ×s.
    seen: 이번 패스에서 이미 ×s한 출력 accessor — 여러 채널이 샘플러 출력을
    공유해도(같은 서브트리 = 같은 ×s) 1회만 스케일한다. 패스별 셋이라 중첩
    스케일 노드는 순차 누적(×s1·s2)이고, 서로 다른 스케일 공간이 하나의
    accessor를 공유하는 비정상 파일은 첫 패스 기준으로만 보정된다."""
    for anim in gltf.get("animations", []):
        samplers = anim.get("samplers", [])
        for ch in anim.get("channels", []):
            tgt = ch.get("target", {})
            if tgt.get("node") in node_ids and tgt.get("path") == "translation":
                out_acc = samplers[ch["sampler"]]["output"]
                if out_acc in seen:
                    continue
                seen.add(out_acc)
                base, stride, count, comp_size, n_comp = accessor_span(gltf, out_acc)
                assert comp_size == 4 and n_comp == 3, "translation 샘플이 f32×3가 아님"
                for i in range(count):
                    o = base + i * stride
                    x, y, z = struct.unpack_from("<fff", bin_bytes, o)
                    struct.pack_into("<fff", bin_bytes, o, x * s, y * s, z * s)


def subtree_nodes(gltf, roots):
    """roots(인덱스 집합)의 자손 전체(자신 포함) 수집."""
    children = {
        i: n.get("children", []) for i, n in enumerate(gltf.get("nodes", []))
    }
    seen, stack = set(), list(roots)
    while stack:
        cur = stack.pop()
        if cur in seen:
            continue
        seen.add(cur)
        stack.extend(children.get(cur, []))
    return seen


def normalize_uniform(path, factor):
    """--uniform: 전체 씬 POSITION·translation 일괄 ×factor (skin 있으면 거부)."""
    gltf, bin_bytes = read_glb(path)
    if gltf.get("skins"):
        raise SystemExit(f"{path}: skin 포함 — --uniform는 비스킨 메시 전용")
    nodes = gltf.get("nodes", [])
    seen = set()
    for d, n in enumerate(nodes):
        if "translation" in n:
            n["translation"] = [v * factor for v in n["translation"]]
        if "mesh" in n:
            scale_positions(gltf, bin_bytes, n["mesh"], factor, seen)
    write_glb(path, gltf, bin_bytes)
    print(f"{path}: 전체 ×{factor} 적용 저장")


def normalize(path):
    gltf, bin_bytes = read_glb(path)
    nodes = gltf.get("nodes", [])
    changed = False
    # 바깥쪽(얕은) 노드부터 — 중첩 스케일도 순차 베이크가 동치
    scaled = [
        i
        for i, n in enumerate(nodes)
        if isinstance(n.get("scale"), list)
        and len(n["scale"]) == 3
        and abs(n["scale"][0] - n["scale"][1]) < 1e-6
        and abs(n["scale"][1] - n["scale"][2]) < 1e-6
        and abs(n["scale"][0] - 1.0) > 1e-4
    ]
    scaled.sort(key=lambda i: depth(gltf, i))
    seen_pos, seen_ibm = set(), set()
    for ni in scaled:
        s = nodes[ni]["scale"][0]
        sub = subtree_nodes(gltf, {ni})
        desc = sub - {ni}
        # 1) 자손 translation ×s
        for d in desc:
            if "translation" in nodes[d]:
                nodes[d]["translation"] = [v * s for v in nodes[d]["translation"]]
        # 2) 서브트리 메시 POSITION ×s
        for d in sub:
            if "mesh" in nodes[d]:
                scale_positions(gltf, bin_bytes, nodes[d]["mesh"], s, seen_pos)
        # 3) 서브트리 메시가 참조하는 skin IBM ÷s
        skin_ids = {
            nodes[d]["skin"] for d in sub if "skin" in nodes[d]
        }
        for si in skin_ids:
            scale_inverse_bindposes(gltf, bin_bytes, si, s, seen_ibm)
        # 4) 애니메이션 translation 채널 ×s — 패스별 seen(중첩 누적 허용)
        scale_anim_translations_indexed(gltf, bin_bytes, sub, s, set())
        # 5) 노드 스케일 제거
        del nodes[ni]["scale"]
        changed = True
        print(f"  node[{ni}] {nodes[ni].get('name', '?')}: scale {s} 베이크")
    if changed:
        write_glb(path, gltf, bin_bytes)
        print(f"{path}: 정규화 저장")
    else:
        print(f"{path}: 스케일 노드 없음 — 무변화")


def depth(gltf, idx):
    parents = {}
    for i, n in enumerate(gltf.get("nodes", [])):
        for c in n.get("children", []):
            parents[c] = i
    d, cur = 0, idx
    while cur in parents:
        d += 1
        cur = parents[cur]
    return d


if __name__ == "__main__":
    args = sys.argv[1:]
    factor = None
    if args and args[0] == "--uniform":
        if len(args) < 3:
            raise SystemExit("--uniform 사용법: --uniform SCALE file.glb ...")
        factor = float(args[1])
        args = args[2:]
    if not args:
        print(__doc__)
        raise SystemExit("파일 인자 필요")
    for p in args:
        if factor is not None:
            normalize_uniform(p, factor)
        else:
            normalize(p)
