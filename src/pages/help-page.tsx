import { useTranslation } from "react-i18next";
import { Link2, KeyRound, MousePointerClick, Rocket, CircleCheck } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";

/** 帮助页：固定域名（Cloudflare Named Tunnel）图文指南 + 常见问题。 */
export function HelpPage() {
  const { t } = useTranslation();

  const steps = [
    {
      icon: KeyRound,
      title: t("help.step1Title"),
      body: t("help.step1Body"),
    },
    {
      icon: MousePointerClick,
      title: t("help.step2Title"),
      body: t("help.step2Body"),
    },
    {
      icon: Rocket,
      title: t("help.step3Title"),
      body: t("help.step3Body"),
    },
  ];

  const perms = [
    { scope: t("help.permAccount"), name: "Cloudflare Tunnel", level: t("help.permEdit") },
    { scope: t("help.permZone"), name: "DNS", level: t("help.permEdit") },
    { scope: t("help.permZone"), name: "Zone", level: t("help.permRead") },
  ];

  const faqs = [
    { q: t("help.faq1Q"), a: t("help.faq1A") },
    { q: t("help.faq2Q"), a: t("help.faq2A") },
    { q: t("help.faq3Q"), a: t("help.faq3A") },
    { q: t("help.faq4Q"), a: t("help.faq4A") },
    { q: t("help.faq5Q"), a: t("help.faq5A") },
  ];

  return (
    <div className="mx-auto max-w-3xl pb-16">
      <h1 className="text-2xl font-semibold tracking-tight">{t("help.title")}</h1>
      <p className="mt-1 text-sm text-muted-foreground">{t("help.subtitle")}</p>

      {/* 两种模式对比 */}
      <div className="mt-6 grid grid-cols-2 gap-3">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="flex items-center gap-2 text-[15px]">
              {t("help.modeTempTitle")}
              <Badge variant="secondary">{t("help.badgeDefault")}</Badge>
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-1.5 text-sm text-muted-foreground">
            <p>{t("help.modeTempDesc")}</p>
            <p className="font-mono text-xs">https://word-word-word.trycloudflare.com</p>
          </CardContent>
        </Card>
        <Card className="border-success/40">
          <CardHeader className="pb-2">
            <CardTitle className="flex items-center gap-2 text-[15px]">
              {t("help.modeFixedTitle")}
              <Badge className="bg-success/15 text-success hover:bg-success/15">
                {t("help.badgeFixed")}
              </Badge>
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-1.5 text-sm text-muted-foreground">
            <p>{t("help.modeFixedDesc")}</p>
            <p className="font-mono text-xs">https://pier.example.com</p>
          </CardContent>
        </Card>
      </div>

      {/* 获取 Token 三步 */}
      <h2 className="mt-10 text-lg font-semibold tracking-tight">
        {t("help.getTokenTitle")}
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">{t("help.getTokenSubtitle")}</p>

      <div className="mt-4 space-y-3">
        {steps.map(({ icon: Icon, title, body }, i) => (
          <Card key={i}>
            <CardContent className="flex items-start gap-4 p-4">
              <div className="flex size-9 shrink-0 items-center justify-center rounded-full bg-accent">
                <Icon className="size-[18px] text-accent-foreground" strokeWidth={1.8} />
              </div>
              <div className="min-w-0 space-y-1.5">
                <p className="font-medium">
                  <span className="mr-2 inline-flex size-5 items-center justify-center rounded-full bg-primary text-[11px] font-semibold text-primary-foreground">
                    {i + 1}
                  </span>
                  {title}
                </p>
                <p className="text-sm leading-relaxed text-muted-foreground">{body}</p>
                {i === 0 && (
                  <Button asChild size="sm" variant="outline" className="mt-1">
                    <a href="https://dash.cloudflare.com/profile/api-tokens" target="_blank" rel="noreferrer">
                      <Link2 className="size-3.5" />
                      {t("help.openTokenPage")}
                    </a>
                  </Button>
                )}
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      {/* 权限清单 */}
      <Card className="mt-4">
        <CardContent className="p-4">
          <p className="mb-2 flex items-center gap-2 text-sm font-medium">
            <CircleCheck className="size-4 text-success" />
            {t("help.permTitle")}
          </p>
          <div className="overflow-hidden rounded-lg border">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b bg-muted/40 text-left text-xs text-muted-foreground">
                  <th className="px-3 py-2 font-medium">{t("help.permScope")}</th>
                  <th className="px-3 py-2 font-medium">{t("help.permName")}</th>
                  <th className="px-3 py-2 font-medium">{t("help.permLevelCol")}</th>
                </tr>
              </thead>
              <tbody>
                {perms.map((p, i) => (
                  <tr key={i} className="border-b last:border-b-0">
                    <td className="px-3 py-2">{p.scope}</td>
                    <td className="px-3 py-2 font-mono text-xs">{p.name}</td>
                    <td className="px-3 py-2">{p.level}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="mt-2 text-xs text-muted-foreground">{t("help.permFooter")}</p>
        </CardContent>
      </Card>

      {/* 常见问题 */}
      <h2 className="mt-10 text-lg font-semibold tracking-tight">{t("help.faqTitle")}</h2>
      <div className="mt-4 space-y-3">
        {faqs.map(({ q, a }, i) => (
          <Card key={i}>
            <CardContent className="p-4">
              <p className="font-medium">{q}</p>
              <p className="mt-1.5 text-sm leading-relaxed text-muted-foreground">{a}</p>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
