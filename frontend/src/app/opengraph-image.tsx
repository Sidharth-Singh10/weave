import { ImageResponse } from "next/og";

export const alt = "Weave, notes into a living knowledge graph";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

/**
 * Social card. Deliberately typographic: the product's own graph geometry is
 * echoed by the nodes on the right, without pretending to be a screenshot.
 */
export default function OpengraphImage() {
  return new ImageResponse(
    (
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
          position: "relative",
          overflow: "hidden",
          background: "#08090a",
          color: "#f2f4f5",
          padding: "72px",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
          <div
            style={{
              width: 48,
              height: 48,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              borderRadius: 15,
              background: "#15181c",
              border: "1px solid #24282e",
              color: "#34d399",
              fontSize: 26,
              fontWeight: 700,
            }}
          >
            W
          </div>
          <div style={{ display: "flex", fontSize: 30, letterSpacing: -1 }}>
            Weave
          </div>
        </div>

        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: 22,
            maxWidth: 840,
          }}
        >
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              fontSize: 78,
              lineHeight: 1.04,
              letterSpacing: -4,
              fontWeight: 700,
              maxWidth: 760,
            }}
          >
            <span style={{ display: "flex" }}>Notes in.</span>
            <span style={{ display: "flex" }}>A living graph out.</span>
          </div>
          <div
            style={{
              display: "flex",
              fontSize: 28,
              lineHeight: 1.35,
              color: "#a8b0ba",
              maxWidth: 760,
            }}
          >
            Concepts, relationships, and the sentence behind every edge.
          </div>
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 14,
            color: "#6f7885",
            fontSize: 20,
          }}
        >
          <div
            style={{ width: 8, height: 8, borderRadius: 4, background: "#34d399" }}
          />
          Deterministic extraction, hybrid retrieval, 18 MCP tools
        </div>

        <div
          style={{
            position: "absolute",
            right: 48,
            top: 175,
            width: 300,
            height: 280,
            display: "flex",
          }}
        >
          <div
            style={{
              position: "absolute",
              left: 20,
              top: 140,
              width: 260,
              height: 1,
              background: "#24282e",
              transform: "rotate(-18deg)",
              transformOrigin: "left center",
            }}
          />
          <div
            style={{
              position: "absolute",
              left: 55,
              top: 150,
              width: 240,
              height: 1,
              background: "#24282e",
              transform: "rotate(22deg)",
              transformOrigin: "left center",
            }}
          />
          <div
            style={{
              position: "absolute",
              left: 10,
              top: 125,
              width: 22,
              height: 22,
              borderRadius: 11,
              background: "#0e1013",
              border: "2px solid #34d399",
            }}
          />
          <div
            style={{
              position: "absolute",
              left: 250,
              top: 60,
              width: 16,
              height: 16,
              borderRadius: 8,
              background: "#0e1013",
              border: "2px solid #6f7885",
            }}
          />
          <div
            style={{
              position: "absolute",
              left: 270,
              top: 210,
              width: 26,
              height: 26,
              borderRadius: 13,
              background: "#0e1013",
              border: "2px solid #34d399",
            }}
          />
          <div
            style={{
              position: "absolute",
              left: 120,
              top: 245,
              width: 14,
              height: 14,
              borderRadius: 7,
              background: "#0e1013",
              border: "2px solid #f5b14c",
            }}
          />
          <div
            style={{
              position: "absolute",
              left: 150,
              top: 120,
              width: 12,
              height: 12,
              borderRadius: 6,
              background: "#34d399",
            }}
          />
        </div>
      </div>
    ),
    size
  );
}
