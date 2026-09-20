import type { SVGProps } from "react";

export const splitMarkPath = "M20 4H58Q60 4 60 6V20Q60 22 58 22H28L50 32C64 38 63 60 44 60H6Q4 60 4 58V44Q4 42 6 42H36L14 32C0 26 1 4 20 4Z";

export default function Logo({ size = 24, ...props }: SVGProps<SVGSVGElement> & { size?: number }) {
  return (
    <svg viewBox="0 0 64 64" width={size} height={size} fill="currentColor" aria-hidden="true" {...props}>
      <path d={splitMarkPath} />
    </svg>
  );
}
