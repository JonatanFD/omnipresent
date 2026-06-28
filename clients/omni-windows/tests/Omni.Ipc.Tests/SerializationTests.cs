using System.Text.Json;
using Omni.Ipc;
using Omni.Ipc.Json;

namespace Omni.Ipc.Tests;

/// <summary>
/// Pins the wire format to the daemon's serde output (see
/// <c>crates/omni-runtime/src/ipc.rs</c>). If these drift, the GUI silently stops
/// talking to the daemon, so the shapes are asserted exactly.
/// </summary>
public class SerializationTests
{
    private static string Serialize(OmniRequest request) =>
        JsonSerializer.Serialize(request, OmniJson.Options);

    [Fact]
    public void Requests_serialize_with_a_cmd_tag()
    {
        Assert.Equal("{\"cmd\":\"hello\"}", Serialize(new HelloRequest()));
        Assert.Equal("{\"cmd\":\"subscribe\"}", Serialize(new SubscribeRequest()));
        Assert.Equal("{\"cmd\":\"status\"}", Serialize(new StatusRequest()));
        Assert.Equal("{\"cmd\":\"stop\"}", Serialize(new StopRequest()));
        Assert.Equal("{\"cmd\":\"peers\"}", Serialize(new PeersRequest()));
        Assert.Equal("{\"cmd\":\"connect\",\"host\":\"10.0.0.2:4733\"}", Serialize(new ConnectRequest("10.0.0.2:4733")));
        Assert.Equal("{\"cmd\":\"disconnect\",\"host\":\"mac\"}", Serialize(new DisconnectRequest("mac")));
        Assert.Equal("{\"cmd\":\"accept\",\"selector\":\"ab12\"}", Serialize(new AcceptRequest("ab12")));
        Assert.Equal("{\"cmd\":\"reject\",\"selector\":\"ab12\"}", Serialize(new RejectRequest("ab12")));
        Assert.Equal("{\"cmd\":\"remove_peer\",\"selector\":\"laptop\"}", Serialize(new RemovePeerRequest("laptop")));
        Assert.Equal("{\"cmd\":\"clipboard\",\"enabled\":true}", Serialize(new ClipboardRequest(true)));
    }

    [Fact]
    public void Layout_request_carries_both_fields_even_when_null()
    {
        // Both null means "list placements"; both set means "place this peer".
        Assert.Equal("{\"cmd\":\"layout\",\"host\":null,\"edge\":null}", Serialize(new LayoutRequest(null, null)));
        Assert.Equal("{\"cmd\":\"layout\",\"host\":\"mac\",\"edge\":\"left\"}", Serialize(new LayoutRequest("mac", "left")));
    }

    [Fact]
    public void Requests_round_trip_as_a_single_line()
    {
        OmniRequest[] requests =
        [
            new HelloRequest(),
            new ConnectRequest("10.0.0.2:4733"),
            new AcceptRequest("ab12"),
            new RemovePeerRequest("laptop"),
            new LayoutRequest("mac", "right"),
            new ClipboardRequest(false),
        ];
        foreach (var request in requests)
        {
            var line = Serialize(request);
            Assert.DoesNotContain('\n', line);
            var back = JsonSerializer.Deserialize<OmniRequest>(line, OmniJson.Options);
            Assert.Equal(request, back);
        }
    }

    [Fact]
    public void Ok_and_error_responses_deserialize()
    {
        Assert.IsType<OkResponse>(JsonSerializer.Deserialize<OmniResponse>("{\"result\":\"ok\"}", OmniJson.Options));

        var error = JsonSerializer.Deserialize<OmniResponse>("{\"result\":\"error\",\"message\":\"boom\"}", OmniJson.Options);
        Assert.Equal(new ErrorResponse("boom"), error);
    }

    [Fact]
    public void Hello_response_deserializes()
    {
        var hello = JsonSerializer.Deserialize<OmniResponse>(
            "{\"result\":\"hello\",\"protocol_version\":1,\"daemon_version\":\"0.3.7\"}", OmniJson.Options);
        Assert.Equal(new HelloResponse(1, "0.3.7"), hello);
    }

    [Fact]
    public void Status_response_deserializes_with_flattened_fields()
    {
        const string line =
            "{\"result\":\"status\",\"fingerprint\":\"ab\",\"port\":4733,\"capturing\":true," +
            "\"clipboard_sharing\":false," +
            "\"sessions\":[{\"host\":\"mac\",\"fingerprint\":\"cd\",\"role\":\"controller\",\"active\":true}]," +
            "\"pending\":[{\"host\":\"win\",\"fingerprint\":\"ef\"}]}";

        var response = Assert.IsType<StatusResponse>(JsonSerializer.Deserialize<OmniResponse>(line, OmniJson.Options));
        var status = response.Status;
        Assert.Equal("ab", status.Fingerprint);
        Assert.Equal(4733, status.Port);
        Assert.True(status.Capturing);
        Assert.False(status.ClipboardSharing);
        Assert.Equal(new SessionInfo("mac", "cd", "controller", true), Assert.Single(status.Sessions));
        Assert.Equal(new PendingInfo("win", "ef"), Assert.Single(status.Pending));
    }

    [Fact]
    public void Peers_and_layout_responses_deserialize()
    {
        var peers = Assert.IsType<PeersResponse>(JsonSerializer.Deserialize<OmniResponse>(
            "{\"result\":\"peers\",\"peers\":[{\"host\":\"mac\",\"fingerprint\":\"cd\",\"connected\":true}," +
            "{\"host\":null,\"fingerprint\":\"ef\",\"connected\":false}]}", OmniJson.Options));
        Assert.Equal(2, peers.Peers.Count);
        Assert.Equal(new PeerInfo("mac", "cd", true), peers.Peers[0]);
        Assert.Equal(new PeerInfo(null, "ef", false), peers.Peers[1]);

        var layout = Assert.IsType<LayoutResponse>(JsonSerializer.Deserialize<OmniResponse>(
            "{\"result\":\"layout\",\"placements\":[{\"host\":\"mac\",\"edge\":\"left\",\"connected\":true}]}",
            OmniJson.Options));
        Assert.Equal(new LayoutInfo("mac", "left", true), Assert.Single(layout.Placements));
    }

    [Fact]
    public void Status_event_is_tagged_event_and_flattens_status()
    {
        var status = new StatusInfo("ab", 4733, false, false, [], []);
        var line = JsonSerializer.Serialize<OmniEvent>(new StatusEvent(status), OmniJson.Options);

        // The "event" tag is what tells a subscriber a push from a response.
        Assert.Contains("\"event\":\"status\"", line);
        Assert.Contains("\"fingerprint\":\"ab\"", line);
        Assert.DoesNotContain('\n', line);

        var back = Assert.IsType<StatusEvent>(JsonSerializer.Deserialize<OmniEvent>(line, OmniJson.Options));
        // Records compare collections by reference, so assert the fields directly.
        Assert.Equal(status.Fingerprint, back.Status.Fingerprint);
        Assert.Equal(status.Port, back.Status.Port);
        Assert.Empty(back.Status.Sessions);
        Assert.Empty(back.Status.Pending);
    }

    [Fact]
    public void Responses_round_trip_through_the_converter()
    {
        OmniResponse[] responses =
        [
            new OkResponse(),
            new ErrorResponse("nope"),
            new HelloResponse(1, "0.3.7"),
            new StatusResponse(new StatusInfo("ab", 4733, true, true,
                [new SessionInfo("mac", "cd", "target", false)],
                [])),
            new PeersResponse([new PeerInfo("mac", "cd", true)]),
            new LayoutResponse([new LayoutInfo("mac", "bottom", false)]),
        ];
        foreach (var response in responses)
        {
            // Round-trip fidelity is asserted structurally (re-serialization),
            // since record equality compares collection fields by reference.
            var line = JsonSerializer.Serialize(response, OmniJson.Options);
            var back = JsonSerializer.Deserialize<OmniResponse>(line, OmniJson.Options);
            var again = JsonSerializer.Serialize(back, OmniJson.Options);
            Assert.Equal(line, again);
        }
    }
}
