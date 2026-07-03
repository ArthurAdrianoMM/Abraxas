import { useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { StoredMessage } from "../../lib/tauri/bindings";
import { useGenerationStore } from "../../stores/generation";
import { Seal } from "./Seal";
import styles from "./Thread.module.css";

function AssistantBody({ content }: { content: string }) {
  return <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>;
}

function Turn({ message }: { message: StoredMessage }) {
  const isUser = message.role === "user";
  return (
    <article className={isUser ? "turn user" : "turn"}>
      <div className="who">
        {!isUser && <Seal />}
        <span>{isUser ? "você" : "abraxas"}</span>
      </div>
      <div className="msg">
        {isUser ? <p>{message.content}</p> : <AssistantBody content={message.content} />}
      </div>
    </article>
  );
}

/** The in-flight assistant turn: typing dots until the first token, then the
 *  streamed text rendered live. */
function LiveTurn() {
  const streamText = useGenerationStore((s) => s.streamText);
  return (
    <article className="turn">
      <div className="who">
        <Seal />
        <span>
          abraxas <span className={styles.thinking}>— pensando devagar</span>
        </span>
      </div>
      <div className="msg">
        {streamText ? (
          <AssistantBody content={streamText} />
        ) : (
          <span className="typing" aria-label="modelo digitando">
            <i></i>
            <i></i>
            <i></i>
          </span>
        )}
      </div>
    </article>
  );
}

export function Thread({ messages }: { messages: StoredMessage[] }) {
  const status = useGenerationStore((s) => s.status);
  const streamText = useGenerationStore((s) => s.streamText);
  const scrollRef = useRef<HTMLElement>(null);

  // Follow the conversation as turns land and tokens stream in.
  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages, status, streamText]);

  return (
    <section className="thread" ref={scrollRef}>
      <div className="thread-inner">
        {messages.map((m) => (
          <Turn key={m.id} message={m} />
        ))}
        {status !== "idle" && <LiveTurn />}
      </div>
    </section>
  );
}
