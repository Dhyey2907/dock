import { Component, type ErrorInfo, type ReactNode } from "react";
import { TriangleAlert } from "lucide-react";

/** Keeps one widget's crash to that widget: the rest of the dock keeps drawing. */
export class WidgetBoundary extends Component<{ name: string; children: ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(`the ${this.props.name} widget failed`, error, info.componentStack);
  }

  render() {
    if (this.state.failed) {
      return (
        <span className="widget-failed" title={`${this.props.name} couldn't be shown`}>
          <TriangleAlert size={16} aria-hidden="true" />
        </span>
      );
    }
    return this.props.children;
  }
}
