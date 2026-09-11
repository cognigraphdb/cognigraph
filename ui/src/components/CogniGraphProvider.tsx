import { CaretDown, Check, CircleNotch, DotsThree, Eye, EyeSlash, X } from "@phosphor-icons/react";
import { App as AntApp, ConfigProvider } from "antd";
import type { PropsWithChildren } from "react";
import { cognigraphTheme } from "../antd-theme.ts";

export function CogniGraphProvider({ children }: PropsWithChildren) {
  return (
    <ConfigProvider
      button={{ loadingIcon: <CircleNotch className="cg-spin" weight="bold" /> }}
      input={{ allowClear: { clearIcon: <X /> } }}
      inputPassword={{ iconRender: (visible) => (visible ? <Eye /> : <EyeSlash />) }}
      modal={{ closeIcon: <X /> }}
      select={{ menuItemSelectedIcon: <Check />, suffixIcon: <CaretDown /> }}
      spin={{ indicator: <CircleNotch className="cg-spin" weight="bold" /> }}
      tabs={{ moreIcon: <DotsThree weight="bold" /> }}
      theme={cognigraphTheme}
    >
      <AntApp>{children}</AntApp>
    </ConfigProvider>
  );
}
