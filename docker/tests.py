import subprocess
import decorator
import json

def exec_base(num):
    return ["docker-compose", "exec", "-T", "--index="+str(num)]


def run_on_hub(command, num=1):
    return subprocess.check_output(exec_base(num) + ["hub", "./gu-hub"] + command)


def run_on_provider(command, num=1):
    return subprocess.check_output(exec_base(num) + ["provider", "./gu-provider"] + command)


def check_ip(name, num):
    return subprocess.check_output(["docker", "inspect", "docker_"+name+"_"+str(num), "--format='{{.NetworkSettings.Networks.docker_network.IPAddress}}'"])[1:-2]


def check_hub_ip(num=1):
    return check_ip("hub", num)


def check_provider_ip(num=1):
    return check_ip("provider", num)


def setup(hubs=1, providers=5):
    def wrapper(func):
        subprocess.call(["docker-compose", "up", "--scale", "provider="+str(providers), "--scale", "hub="+str(hubs), "--build", "--no-recreate", "-d"])
        func()
        subprocess.call(["docker-compose", "down"])

    return decorator.decorator(wrapper)


@setup(hubs=1, providers=1)
def _test_autocomplete():
    run_on_provider(["completions", "bash"])
    run_on_provider(["completions", "zsh"])
    run_on_hub(["completions", "fish"])


@setup(hubs=1, providers=1)
def _test_daemonize():
    assert run_on_provider(["server", "status"]) == 'Process is running (pid: 1)\n'


@setup(hubs=2, providers=6)
def _test_mdns_listing():
    lan = json.loads(run_on_provider(["--json", "lan", "list"]))
    assert len(lan) == 8

    lan = json.loads(run_on_provider(["--json", "lan", "list", "-I", "gu-hub"]))
    assert len(lan) == 2

    lan = json.loads(run_on_provider(["--json", "lan", "list", "-I", "gu-provider"]))
    assert len(lan) == 6


@setup(hubs=2, providers=3)
def _test_mdns_autoconnect():
    run_on_provider(["hubs", "auto"], num=1)
    run_on_provider(["hubs", "auto"], num=2)

    hubs = json.loads(run_on_provider(["--json", "hubs", "list"], num=1))
    assert len(hubs) == 2

    hubs = json.loads(run_on_provider(["--json", "hubs", "list"], num=2))
    assert len(hubs) == 2

    peers = json.loads(run_on_hub(["--json", "peer", "list"], num=1))
    assert len(peers) == 2

    peers = json.loads(run_on_hub(["--json", "peer", "list"], num=2))
    assert len(peers) == 2


@setup(hubs=2, providers=3)
def _test_connect():
    hub_ip = check_hub_ip()

    # connect
    run_on_provider(["hubs", "connect", hub_ip+":61622"])
    hubs = json.loads(run_on_provider(["--json", "hubs", "list"]))
    assert len(hubs) == 1

    # disconnect
    run_on_provider(["hubs", "disconnect", hub_ip+":61622"])
    assert run_on_provider(["--json", "hubs", "list"]) == ""


@setup(hubs=1, providers=0)
def _test_plugin():
    subprocess.call(["docker", "cp", "Factorization.gu-plugin", "docker_hub_1:/Factor.gu-plugin"])

    # install plugin
    run_on_hub(["plugin", "install", "/Factor.gu-plugin"])
    plugins = json.loads(run_on_hub(["--json", "plugin", "list"]))
    assert len(plugins) == 1
    assert plugins[0]["Status"] == "Active"

    # stop plugin
    run_on_hub(["plugin", "stop", "Factorization"])
    plugins = json.loads(run_on_hub(["--json", "plugin", "list"]))
    assert len(plugins) == 1
    assert plugins[0]["Status"] == "Installed"

    # start plugin
    run_on_hub(["plugin", "start", "Factorization"])
    plugins = json.loads(run_on_hub(["--json", "plugin", "list"]))
    assert len(plugins) == 1
    assert plugins[0]["Status"] == "Active"

    # uninstall plugin
    run_on_hub(["plugin", "uninstall", "Factorization"])
    assert run_on_hub(["--json", "plugin", "list"]) == ""


# FIXME recognize command error
@setup(hubs=1, providers=1)
def test_group():
    hub_ip = check_hub_ip()
    hub_node_id = json.loads(run_on_provider(["--json", "lan", "list"]))[0]["Description"][8:]
    assert run_on_provider(["join", hub_node_id, hub_ip+":61622"]) == ""
