import { memo } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { useI18n } from "../i18n/context";

function ImageLabel({ alt }: { alt?: string }) {
  const { t } = useI18n();
  return (
    <span className="image-label">
      {alt ? t("transcript.imageAlt", { alt }) : t("transcript.image")}
    </span>
  );
}

// Links are opened by installExternalLinkHandler. Remote images are blocked by the CSP for
// privacy, so they're shown as labels instead of broken images.
const components: Components = {
  img: ({ alt }) => <ImageLabel alt={alt} />,
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
