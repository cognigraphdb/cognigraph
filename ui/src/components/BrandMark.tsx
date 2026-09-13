import mark from "../assets/cognigraph-mark.svg";

/** The product mark uses an alpha mask so each surface supplies its own color. */
export function BrandMark() {
  return (
    <span
      aria-hidden="true"
      className="brand-mark"
      style={{ maskImage: `url("${mark}")`, WebkitMaskImage: `url("${mark}")` }}
    />
  );
}
