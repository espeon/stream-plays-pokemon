import { useState, useEffect, useRef, useCallback } from "react";
import {
  lookupLocation,
  lookupMapDimensions,
  emeraldMapToObject,
  MAPSEC,
  REGION_MAP_TILE_W,
  REGION_MAP_TILE_H,
} from "../emerald-map-data";
import type { PlayerLocation } from "../types";
import { CrossFade } from "react-crossfade-simple";

const METATILE_PX = 16;
const MAX_SCALE = 4;

function mapsecColor(id: number, active: boolean): string {
  if (active) return "#f97316";
  if (id <= 6) return "#d4d4aa"; // towns
  if (id <= 15) return "#e8e8c0"; // cities
  if ((id >= 22 && id <= 24) || (id >= 39 && id <= 49)) return "#4a8fb5"; // sea routes
  if (id >= 50 && id <= 54) return "#2d6080"; // underwater
  if (id >= 16 && id <= 49) return "#4a7c50"; // land routes
  return "#6b6b6b"; // dungeons / special
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function useContainerSize(ref: React.RefObject<HTMLDivElement | null>) {
  const [size, setSize] = useState({ w: 0, h: 0 });

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) {
        setSize({
          w: entry.contentRect.width,
          h: entry.contentRect.height,
        });
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref]);

  return size;
}

interface CameraResult {
  scale: number;
  translateX: number;
  translateY: number;
}

function computeCamera(
  containerW: number,
  containerH: number,
  mapPxW: number,
  mapPxH: number,
  playerPxX: number,
  playerPxY: number,
): CameraResult {
  // Scale to cover the viewport (fill at least one dimension)
  const scaleX = containerW / mapPxW;
  const scaleY = containerH / mapPxH;
  const scale = Math.min(Math.max(scaleX, scaleY), MAX_SCALE);

  const scaledW = mapPxW * scale;
  const scaledH = mapPxH * scale;

  // If scaled map fits in container, center it (no panning needed)
  const maxOffsetX = Math.max(0, scaledW - containerW);
  const maxOffsetY = Math.max(0, scaledH - containerH);

  // Ideal offset: center on the player
  const idealX = playerPxX * scale - containerW / 2;
  const idealY = playerPxY * scale - containerH / 2;

  // Clamp so map edges don't leave empty space
  const translateX =
    maxOffsetX > 0 ? clamp(idealX, 0, maxOffsetX) : (scaledW - containerW) / 2;
  const translateY =
    maxOffsetY > 0 ? clamp(idealY, 0, maxOffsetY) : (scaledH - containerH) / 2;

  return { scale, translateX, translateY };
}

export function MapPanel({ location }: { location: PlayerLocation | null }) {
  const [imgError, setImgError] = useState(false);
  const viewportRef = useRef<HTMLDivElement>(null);
  const containerSize = useContainerSize(viewportRef);
  const prevMapId = useRef<string | null>(null);
  const [animate, setAnimate] = useState(false);

  const entry = location
    ? lookupLocation(location.map_bank, location.map_num)
    : null;
  const entryObj = location
    ? emeraldMapToObject(location.map_bank, location.map_num)
    : null;

  const activeMapsecId = entry
    ? (Object.entries(MAPSEC).find(([, v]) => v === entry)?.[0] ?? null)
    : null;

  const activeName = entry
    ? entry.name
    : location
      ? `Map ${location.map_bank}:${location.map_num}`
      : "—";

  const mapId = location
    ? ((location.map_bank << 8) | location.map_num)
        .toString(16)
        .padStart(4, "0")
        .toUpperCase()
    : null;

  const dims = location
    ? lookupMapDimensions(location.map_bank, location.map_num)
    : null;

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setImgError(false);
    // Disable transition on map change to avoid sliding from old position
    if (mapId !== prevMapId.current) {
      setAnimate(false);
      prevMapId.current = mapId;
      // Re-enable transitions after a frame
      requestAnimationFrame(() => setAnimate(true));
    }
  }, [mapId]);

  // Compute camera transform
  const camera = useCallback((): CameraResult | null => {
    if (!dims || !location || containerSize.w === 0) return null;

    const mapPxW = dims.w * METATILE_PX;
    const mapPxH = dims.h * METATILE_PX;

    // Player position in pixels within the map.
    // SaveBlock1.pos is in metatile coordinates (verified via pokeemerald decomp:
    // event_object_movement.c converts pos to pixels with << 4, i.e. * 16).
    const playerPxX = location.x * METATILE_PX + METATILE_PX / 2;
    const playerPxY = location.y * METATILE_PX + METATILE_PX / 2;

    return computeCamera(
      containerSize.w,
      containerSize.h,
      mapPxW,
      mapPxH,
      playerPxX,
      playerPxY,
    );
  }, [dims, location, containerSize]);

  const cam = camera();

  const VW = REGION_MAP_TILE_W;
  const VH = REGION_MAP_TILE_H;

  return (
    <div className="w-full h-full flex flex-col relative">
      <div className="px-4 pt-2 pb-1 shrink-0 z-100 bg-linear-to-b from-muted via-muted/90 to-transparent">
        <CrossFade
          contentKey={
            entryObj
              ? `${entryObj.mainArea}-${entryObj.mapName}-${entryObj.floor}`
              : "unknown-location"
          }
          timeout={300}
        >
          <div
            className={`text-xl uppercase font-text font-medium tracking-widest text-foreground ${!entryObj?.mapName && "translate-y-2.5"}`}
          >
            {entryObj?.mainArea}
          </div>
          <div className="text-base uppercase tracking-widest text-muted-foreground">
            {entryObj?.mapName}{" "}
            {entryObj?.floor
              ? `(${entryObj.floor < 0 ? `B${entryObj.floor}` : entryObj.floor}F)`
              : ""}
            <span className="text-muted opacity-0">a</span>
          </div>
        </CrossFade>
      </div>

      <div
        ref={viewportRef}
        className="flex-1 relative overflow-hidden rounded-xl border-t border-x -mx-px border-white/12"
      >
        {/* Camera-controlled area map */}
        <CrossFade
          contentKey={`${mapId ?? "no-map"} ${imgError}`}
          timeout={300}
        >
          {mapId && !imgError && dims && cam ? (
            <div
              style={{
                position: "absolute",
                width: dims.w * METATILE_PX,
                height: dims.h * METATILE_PX,
                transformOrigin: "0 0",
                transform: `scale(${cam.scale}) translate(${-cam.translateX / cam.scale}px, ${-cam.translateY / cam.scale}px)`,
                transition: animate ? "transform 0.45s ease-out" : "none",
                imageRendering: "pixelated",
              }}
            >
              <img
                src={`/maps/${mapId}.png`}
                onError={() => setImgError(true)}
                style={{
                  width: "100%",
                  height: "100%",
                  imageRendering: "pixelated",
                }}
                alt={activeName}
              />
              {/* Player position marker */}
              {location && (
                <div
                  className="player-marker"
                  style={{
                    position: "absolute",
                    transform: `translate(${location.x * METATILE_PX}px, ${location.y * METATILE_PX}px)`,
                    left: 0,
                    top: 0,
                    // left: location.x * METATILE_PX + METATILE_PX / 2,
                    // top: location.y * METATILE_PX + METATILE_PX / 2,
                    width: METATILE_PX,
                    height: METATILE_PX,
                    transition: animate ? "transform 0.45s ease-out" : "none",
                    filter: "drop-shadow(0 0 2px #000aa)",
                  }}
                >
                  <img
                    src="/may_icon.png"
                    alt="May Icon"
                    className="w-full h-full"
                  />
                </div>
              )}
            </div>
          ) : (
            <div className="absolute w-full h-full flex items-center justify-center">
              <span className="text-white/15 text-xs uppercase tracking-widest">
                {mapId ? "map unavailable" : "—"}
              </span>
            </div>
          )}
          {/* World map overlay — top left */}
          <div className="absolute top-2 left-2 h-32 rounded opacity-80 overflow-hidden border bg-muted/80">
            <svg
              viewBox={`0 0 ${VW} ${VH}`}
              className="w-full h-full"
              preserveAspectRatio="xMidYMid meet"
            >
              <rect x={0} y={0} width={VW} height={VH} fill="#1e3a5f" />
              {Object.entries(MAPSEC).map(([idStr, sec]) => {
                const id = Number(idStr);
                const isActive = idStr === activeMapsecId;
                return (
                  <rect
                    key={id}
                    x={sec.tx}
                    y={sec.ty}
                    width={sec.tw}
                    height={sec.th}
                    fill={mapsecColor(id, isActive)}
                    opacity={isActive ? 1 : 0.85}
                  />
                );
              })}
              {entry && (
                <rect
                  x={entry.tx}
                  y={entry.ty}
                  width={entry.tw}
                  height={entry.th}
                  fill="none"
                  stroke="white"
                  strokeWidth={0.15}
                />
              )}
            </svg>
          </div>
        </CrossFade>
      </div>
    </div>
  );
}
