/**
 * English note.
 * English note.
 */

// English engineering note.

/**
 * English note.
 */
export interface MemberActivity {
  memberId: number;
  platformId: string;
  name: string;
  messageCount: number;
  percentage: number; // English engineering note.
  avatar?: string | null; // English engineering note.
}

/**
 * English note.
 */
export interface MemberWithStats {
  id: number;
  platformId: string;
  accountName: string | null; // English engineering note.
  groupNickname: string | null; // English engineering note.
  aliases: string[]; // English engineering note.
  messageCount: number;
  avatar: string | null; // English engineering note.
}

/**
 * English note.
 */
export interface HourlyActivity {
  hour: number; // 0-23
  messageCount: number;
}

/**
 * English note.
 */
export interface DailyActivity {
  date: string; // YYYY-MM-DD
  messageCount: number;
}

/**
 * English note.
 */
export interface WeekdayActivity {
  weekday: number; // English engineering note.
  messageCount: number;
}

/**
 * English note.
 */
export interface MonthlyActivity {
  month: number; // 1-12
  messageCount: number;
}

/**
 * English note.
 */
export interface MemberNameHistory {
  nameType: "account_name" | "group_nickname"; // English engineering note.
  name: string; // English engineering note.
  startTs: number; // English engineering note.
  endTs: number | null; // English engineering note.
}

// English engineering note.

/**
 * English note.
 */
export type NightOwlTitle =
  | "养生达人"
  | "偶尔失眠"
  | "经常失眠"
  | "夜猫子"
  | "秃头预备役"
  | "修仙练习生"
  | "守夜冠军";

/**
 * English note.
 */
export interface NightOwlRankItem {
  memberId: number;
  platformId: string;
  name: string;
  totalNightMessages: number; // English engineering note.
  title: NightOwlTitle; // English engineering note.
  hourlyBreakdown: {
    // English engineering note.
    h23: number; // 23:00-24:00
    h0: number; // 00:00-01:00
    h1: number; // 01:00-02:00
    h2: number; // 02:00-03:00
    h3to4: number; // 03:00-05:00
  };
  percentage: number; // English engineering note.
}

/**
 * English note.
 */
export interface TimeRankItem {
  memberId: number;
  platformId: string;
  name: string;
  count: number; // English engineering note.
  avgTime: string; // English engineering note.
  extremeTime: string; // English engineering note.
  percentage: number; // English engineering note.
}

/**
 * English note.
 */
export interface ConsecutiveNightRecord {
  memberId: number;
  platformId: string;
  name: string;
  maxConsecutiveDays: number; // English engineering note.
  currentStreak: number; // English engineering note.
}

/**
 * English note.
 */
export interface NightOwlChampion {
  memberId: number;
  platformId: string;
  name: string;
  score: number; // English engineering note.
  nightMessages: number; // English engineering note.
  lastSpeakerCount: number; // English engineering note.
  consecutiveDays: number; // English engineering note.
}

/**
 * English note.
 */
export interface NightOwlAnalysis {
  // English engineering note.
  nightOwlRank: NightOwlRankItem[];
  // English engineering note.
  lastSpeakerRank: TimeRankItem[];
  // English engineering note.
  firstSpeakerRank: TimeRankItem[];
  // English engineering note.
  consecutiveRecords: ConsecutiveNightRecord[];
  // English engineering note.
  champions: NightOwlChampion[];
  // English engineering note.
  totalDays: number;
}

/**
 * English note.
 */
export interface DragonKingRankItem {
  memberId: number;
  platformId: string;
  name: string;
  count: number; // English engineering note.
  percentage: number; // English engineering note.
}

/**
 * English note.
 */
export interface DragonKingAnalysis {
  rank: DragonKingRankItem[];
  totalDays: number; // English engineering note.
}

/**
 * English note.
 */
export interface DivingRankItem {
  memberId: number;
  platformId: string;
  name: string;
  lastMessageTs: number; // English engineering note.
  daysSinceLastMessage: number; // English engineering note.
}

/**
 * English note.
 */
export interface DivingAnalysis {
  rank: DivingRankItem[];
}

// English engineering note.

/**
 * English note.
 */
export interface RepeatStatItem {
  memberId: number;
  platformId: string;
  name: string;
  count: number; // English engineering note.
  percentage: number; // English engineering note.
}

/**
 * English note.
 */
export interface RepeatRateItem {
  memberId: number;
  platformId: string;
  name: string;
  count: number; // English engineering note.
  totalMessages: number; // English engineering note.
  rate: number; // English engineering note.
}

/**
 * English note.
 */
export interface ChainLengthDistribution {
  length: number; // English engineering note.
  count: number; // English engineering note.
}

/**
 * English note.
 */
export interface HotRepeatContent {
  content: string; // English engineering note.
  count: number; // English engineering note.
  maxChainLength: number; // English engineering note.
  originatorName: string; // English engineering note.
  lastTs: number; // English engineering note.
  firstMessageId: number; // English engineering note.
}

/**
 * English note.
 */
export interface MemberCatchphrase {
  memberId: number;
  platformId: string;
  name: string;
  catchphrases: Array<{
    content: string;
    count: number;
  }>;
}

/**
 * English note.
 */
export interface CatchphraseAnalysis {
  members: MemberCatchphrase[];
}

/**
 * English note.
 */
export interface FastestRepeaterItem {
  memberId: number;
  platformId: string;
  name: string;
  count: number; // English engineering note.
  avgTimeDiff: number; // English engineering note.
}

/**
 * English note.
 */
export interface RepeatAnalysis {
  // English engineering note.
  originators: RepeatStatItem[];
  // English engineering note.
  initiators: RepeatStatItem[];
  // English engineering note.
  breakers: RepeatStatItem[];
  // English engineering note.
  fastestRepeaters: FastestRepeaterItem[];

  // English engineering note.
  originatorRates: RepeatRateItem[];
  // English engineering note.
  initiatorRates: RepeatRateItem[];
  // English engineering note.
  breakerRates: RepeatRateItem[];

  // English engineering note.
  chainLengthDistribution: ChainLengthDistribution[];
  // English engineering note.
  hotContents: HotRepeatContent[];
  // English engineering note.
  avgChainLength: number;

  // English engineering note.
  totalRepeatChains: number;
}

// English engineering note.

/**
 * English note.
 */
export interface MentionRankItem {
  memberId: number;
  platformId: string;
  name: string;
  count: number; // English engineering note.
  percentage: number; // English engineering note.
}

/**
 * English note.
 */
export interface MentionPair {
  fromMemberId: number;
  fromName: string;
  toMemberId: number;
  toName: string;
  count: number; // English engineering note.
}

/**
 * English note.
 */
export interface OneWayMention {
  fromMemberId: number;
  fromName: string;
  toMemberId: number;
  toName: string;
  fromToCount: number; // English engineering note.
  toFromCount: number; // English engineering note.
  ratio: number; // English engineering note.
}

/**
 * English note.
 */
export interface TwoWayMention {
  member1Id: number;
  member1Name: string;
  member2Id: number;
  member2Name: string;
  member1To2: number; // A @ B
  member2To1: number; // B @ A
  total: number; // English engineering note.
  balance: number; // English engineering note.
}

/**
 * English note.
 */
export interface MemberMentionDetail {
  memberId: number;
  name: string;
  // English engineering note.
  topMentioned: MentionPair[];
  // English engineering note.
  topMentioners: MentionPair[];
}

/**
 * English note.
 */
export interface MentionAnalysis {
  // English engineering note.
  topMentioners: MentionRankItem[];
  // English engineering note.
  topMentioned: MentionRankItem[];
  // English engineering note.
  oneWay: OneWayMention[];
  // English engineering note.
  twoWay: TwoWayMention[];
  // English engineering note.
  totalMentions: number;
  // English engineering note.
  memberDetails: MemberMentionDetail[];
}

// English engineering note.

/**
 * English note.
 */
export interface LaughRankItem {
  memberId: number;
  platformId: string;
  name: string;
  laughCount: number; // English engineering note.
  messageCount: number; // English engineering note.
  laughRate: number; // English engineering note.
  percentage: number; // English engineering note.
  keywordDistribution: Array<{
    keyword: string;
    count: number;
    percentage: number;
  }>; // English engineering note.
}

/**
 * English note.
 */
export interface LaughTypeDistribution {
  type: string; // English engineering note.
  count: number; // English engineering note.
  percentage: number; // English engineering note.
}

/**
 * English note.
 */
export interface LaughAnalysis {
  // English engineering note.
  rankByRate: LaughRankItem[];
  // English engineering note.
  rankByCount: LaughRankItem[];
  // English engineering note.
  typeDistribution: LaughTypeDistribution[];
  // English engineering note.
  totalLaughs: number;
  // English engineering note.
  totalMessages: number;
  // English engineering note.
  groupLaughRate: number;
}

// English engineering note.

/**
 * English note.
 */
export interface MemeBattleRankItem {
  memberId: number;
  platformId: string;
  name: string;
  count: number; // English engineering note.
  percentage: number; // English engineering note.
}

/**
 * English note.
 */
export interface MemeBattleRecord {
  startTime: number; // English engineering note.
  endTime: number; // English engineering note.
  totalImages: number; // English engineering note.
  participantCount: number; // English engineering note.
  participants: Array<{
    memberId: number;
    name: string;
    imageCount: number; // English engineering note.
  }>;
}

/**
 * English note.
 */
export interface MemeBattleAnalysis {
  topBattles: MemeBattleRecord[]; // English engineering note.
  rankByCount: MemeBattleRankItem[]; // English engineering note.
  rankByImageCount: MemeBattleRankItem[]; // English engineering note.
  totalBattles: number; // English engineering note.
}

// English engineering note.

/**
 * English note.
 */
export interface StreakRankItem {
  memberId: number;
  name: string;
  maxStreak: number; // English engineering note.
  maxStreakStart: string; // English engineering note.
  maxStreakEnd: string; // English engineering note.
  currentStreak: number; // English engineering note.
}

/**
 * English note.
 */
export interface LoyaltyRankItem {
  memberId: number;
  name: string;
  totalDays: number; // English engineering note.
  percentage: number; // English engineering note.
}

/**
 * English note.
 */
export interface CheckInAnalysis {
  streakRank: StreakRankItem[]; // English engineering note.
  loyaltyRank: LoyaltyRankItem[]; // English engineering note.
  totalDays: number; // English engineering note.
}

// English engineering note.

/**
 * English note.
 */
export interface KeywordTemplate {
  id: string;
  name: string;
  keywords: string[];
}

// English engineering note.

/**
 * English note.
 */
export interface ClusterGraphOptions {
  // English engineering note.
  lookAhead?: number;
  // English engineering note.
  decaySeconds?: number;
  // English engineering note.
  topEdges?: number;
}

/**
 * English note.
 */
export interface ClusterGraphNode {
  id: number;
  name: string;
  messageCount: number;
  symbolSize: number;
  degree: number;
  normalizedDegree: number;
}

/**
 * English note.
 */
export interface ClusterGraphLink {
  source: string;
  target: string;
  value: number;
  rawScore: number;
  expectedScore: number;
  coOccurrenceCount: number;
}

/**
 * English note.
 */
export interface ClusterGraphCommunity {
  id: number;
  name: string;
  size: number;
}

/**
 * English note.
 */
export interface ClusterGraphStats {
  totalMembers: number;
  totalMessages: number;
  involvedMembers: number;
  edgeCount: number;
  communityCount: number;
}

/**
 * English note.
 */
export interface ClusterGraphData {
  nodes: ClusterGraphNode[];
  links: ClusterGraphLink[];
  maxLinkValue: number;
  communities: ClusterGraphCommunity[];
  stats: ClusterGraphStats;
}
