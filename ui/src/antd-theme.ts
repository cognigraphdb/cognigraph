import { type ThemeConfig, theme } from "antd";

export const cognigraphTheme: ThemeConfig = {
  algorithm: theme.compactAlgorithm,
  cssVar: { key: "cognigraph", prefix: "cg" },
  token: {
    // rc-motion transitions are unreliable in the Bun dev bundle: modals
    // intermittently freeze mid-enter (opacity 0) or never finish leaving.
    // A management console doesn't need entrance animation — turn it off
    // and every overlay appears/dismisses deterministically.
    motion: false,
    colorPrimary: "#087f7c",
    colorInfo: "#087f7c",
    colorSuccess: "#1b9b67",
    colorWarning: "#ba7a14",
    colorError: "#d33d42",
    colorText: "#1d2524",
    colorTextSecondary: "#66716f",
    colorBorder: "#cbd2d0",
    colorBorderSecondary: "#dfe4e2",
    colorBgLayout: "#f7f8f6",
    colorBgContainer: "#ffffff",
    colorFillAlter: "#f3f5f3",
    borderRadius: 6,
    controlHeight: 38,
    fontFamily: 'Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
    fontSize: 14,
    boxShadowSecondary: "0 12px 36px rgb(20 33 31 / 12%)",
  },
  components: {
    Button: {
      fontWeight: 600,
      primaryShadow: "none",
      dangerShadow: "none",
    },
    Modal: {
      titleFontSize: 18,
    },
    Table: {
      borderColor: "#dfe4e2",
      headerBg: "#fafbfa",
      headerColor: "#4c5755",
      headerSplitColor: "transparent",
      rowHoverBg: "#f7faf9",
    },
    Tabs: {
      inkBarColor: "#087f7c",
      itemActiveColor: "#056663",
      itemHoverColor: "#056663",
      itemSelectedColor: "#056663",
    },
  },
};
