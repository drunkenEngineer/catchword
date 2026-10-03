// Rendering with an engine, for tests.
import { render } from "@testing-library/react";
import type { ReactElement } from "react";
import { EngineContext, type Engine } from "./engine";

export function renderWith(engine: Engine, element: ReactElement) {
  return render(<EngineContext value={engine}>{element}</EngineContext>);
}
