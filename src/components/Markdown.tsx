import { memo } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

// Links are opened by installExternalLinkHandler. Remote images are blocked by the CSP for
// privacy, so they're shown as labels instead of broken images.
const components: Components = {
  img: ({ alt }) => <span className="image-label">[image{alt ? `: ${alt}` : ""}]</span>,
};

export const Markdown = memo(function Markdown({ text }: { text: string }) {
  return (
    <div className="markdown">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
        {text}
      </ReactMarkdown>
    </div>
  );
});
