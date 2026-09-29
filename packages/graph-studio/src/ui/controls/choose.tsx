/** The control a parameter asks for, and nothing else: every kind is a component of its own. */
import type { ReactElement } from "react";

import { controlOf } from "../controlOf.ts";
import { FileControl } from "./FileControl.tsx";
import { ListControl } from "./ListControl.tsx";
import { NumberControl } from "./NumberControl.tsx";
import type { ControlProps } from "./props.ts";
import { SegmentedControl } from "./SegmentedControl.tsx";
import { SelectControl } from "./SelectControl.tsx";
import { SliderControl } from "./SliderControl.tsx";
import { TextControl } from "./TextControl.tsx";
import { ToggleControl } from "./ToggleControl.tsx";

export function ControlFor(props: ControlProps): ReactElement {
  switch (controlOf(props.spec)) {
    case "list": return <ListControl {...props} />;
    case "select": return <SelectControl {...props} />;
    case "segmented": return <SegmentedControl {...props} />;
    case "slider": return <SliderControl {...props} />;
    case "number": return <NumberControl {...props} />;
    case "text": return <TextControl {...props} />;
    case "toggle": return <ToggleControl {...props} />;
    case "file": return <FileControl {...props} />;
  }
}
