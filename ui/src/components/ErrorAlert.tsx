import { XCircle } from "@phosphor-icons/react";
import { Alert } from "antd";

/// The one way we render an inline error banner. Supplies the phosphor
/// status icon explicitly: antd v6's built-in `showIcon` glyph path trips a
/// dev-mode @ant-design/icons warning, and the explicit icon also keeps the
/// alert consistent with the toast icons in CogniGraphProvider.
export function ErrorAlert({ title }: { title: string }) {
  return <Alert icon={<XCircle weight="fill" />} showIcon title={title} type="error" />;
}
